use agent_desktop_core::{
    AdapterError, AppError, Deadline, DeliverySemantics, ErrorCode, PermissionReport,
    PlatformAdapter, context::CommandContext,
};
use serde_json::{Value, json};

use crate::{cli::Commands, cli_args::batch::BatchArgs, command_policy::CommandEffect};

use super::{
    batch_stop::BatchStop,
    bounded_json::serialized_size,
    entry_errors::{baseline_error, batch_session_ended, batch_timeout, wait_baseline_error},
    entry_outcome::EntryOutcome,
    pending_seed::{PendingSeed, read_seed, seed_context},
    preparation::{MAX_BATCH_ENTRIES, MAX_BATCH_JSON_BYTES, PreparedCommand, prepare},
    result_entry::{MAX_BATCH_OUTPUT_BYTES, not_started_entry},
    seed_plan::SeedPlan,
};

/// Runs the batch and records, in `entry_effects`, the effect of every
/// prepared entry by its index, so a caller that settles the batch later can
/// read each entry's disposition with its effect.
pub(super) fn execute(
    args: BatchArgs,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
    entry_effects: &mut Vec<CommandEffect>,
) -> Result<Value, AppError> {
    let deadline = Deadline::after(args.timeout_ms)
        .map_err(|error| error.with_disposition(DeliverySemantics::not_delivered()))?;
    let batch_context = context.clone().with_inherited_deadline(deadline);
    let mut commands = prepare(&args.commands_json, permission_report, &batch_context)?;
    let total = commands.len();
    entry_effects.extend(commands.iter().map(|prepared| prepared.command.effect()));
    let seed_plan = SeedPlan::new(&commands);
    let mut results = Vec::with_capacity(total);
    let mut results_bytes = 0;
    let mut completed = 0;
    let mut pending_seed: Option<PendingSeed> = None;
    let mut stopped: Option<BatchStop> = None;

    for index in 0..total {
        if deadline.is_expired() {
            let error = batch_timeout(index, &commands[index].name, args.timeout_ms);
            push_small_entry(
                &mut results,
                &mut results_bytes,
                not_started_entry(index, &commands[index].name, "deadline", error),
            );
            stopped = Some(BatchStop::DeadlineBeforeEntry { index });
            break;
        }

        let current_seed = pending_seed.take_if(|seed| seed.is_for(index));

        if let Some(ended_session) = session_ended_for(&commands[index]) {
            let error = batch_session_ended(index, &commands[index].name, &ended_session);
            push_small_entry(
                &mut results,
                &mut results_bytes,
                not_started_entry(index, &commands[index].name, "session_ended", error),
            );
            if args.stop_on_error {
                stopped = Some(BatchStop::StopOnError { index });
                break;
            }
            continue;
        }

        let current_seed = match current_seed.map(PendingSeed::into_read) {
            Some((read_at, Err(error))) => {
                let read_command = &commands[read_at].name;
                let error =
                    wait_baseline_error(index, &commands[index].name, read_at, read_command, error);
                push_small_entry(
                    &mut results,
                    &mut results_bytes,
                    not_started_entry(
                        index,
                        &commands[index].name,
                        "pre_action_baseline_failed",
                        error,
                    ),
                );
                if args.stop_on_error {
                    stopped = Some(BatchStop::StopOnError { index });
                    break;
                }
                continue;
            }
            Some((_, Ok(values))) => Some(values),
            None => None,
        };

        if let Some(wait_index) = seed_plan.read_before(index) {
            match read_seed(&commands[wait_index], adapter, deadline) {
                Err(_) if deadline.is_expired() => {
                    let error = batch_timeout(index, &commands[index].name, args.timeout_ms);
                    push_small_entry(
                        &mut results,
                        &mut results_bytes,
                        not_started_entry(index, &commands[index].name, "deadline", error),
                    );
                    stopped = Some(BatchStop::DeadlineBeforeEntry { index });
                    break;
                }
                Err(error) if wait_index == index + 1 => {
                    let wait_command = commands[wait_index].name.clone();
                    let error = baseline_error(
                        index,
                        &commands[index].name,
                        wait_index,
                        &wait_command,
                        error,
                    );
                    push_small_entry(
                        &mut results,
                        &mut results_bytes,
                        not_started_entry(
                            index,
                            &commands[index].name,
                            "pre_action_baseline_failed",
                            error,
                        ),
                    );
                    stopped = Some(BatchStop::BaselineFailed {
                        blocked_index: index,
                        blocked_command: commands[index].name.clone(),
                        wait_index,
                        wait_command,
                    });
                    break;
                }
                read => pending_seed = PendingSeed::held(wait_index, index, read),
            }
        }

        let item_context = seed_context(
            current_seed,
            commands[index]
                .context
                .clone()
                .with_inherited_deadline(deadline),
        );
        let command = std::mem::replace(&mut commands[index].command, Commands::Version);
        let batch_remaining_ms = deadline.remaining_ms();
        let entry = super::result_entry::completed_entry(
            index,
            &commands[index].name,
            crate::dispatch::dispatch(command, adapter, permission_report, &item_context),
        );
        let outcome = EntryOutcome::of(index, &entry, batch_remaining_ms, deadline.is_expired());
        let (entry, oversized) = super::result_entry::bounded_completed_entry(
            index,
            &commands[index].name,
            entry_effects[index],
            entry,
            results_bytes,
        );
        completed += usize::from(entry["execution"] == "completed");
        push_small_entry(&mut results, &mut results_bytes, entry);

        let outcome = outcome.with_oversized(oversized);
        stopped = BatchStop::after_entry(
            &outcome,
            deadline.is_expired() && index + 1 < total,
            args.stop_on_error,
        );
        if stopped.is_some() {
            break;
        }
        if !outcome.failed
            && let Some(wait_index) = seed_plan.read_again_after(index)
        {
            let read = read_seed(&commands[wait_index], adapter, deadline);
            pending_seed = PendingSeed::held(wait_index, index, read);
        }
    }

    let mut body = json!({
        "results": results,
        "semantics": {
            "atomic": false,
            "order": "sequential",
            "batch_retries": false,
            "command_retry_contracts": "preserved",
            "successful_action_disposition": "data.disposition",
            "error_disposition": "error.disposition",
        },
        "total_entries": total,
        "completed_entries": completed,
        "not_started_entries": total.saturating_sub(completed),
        "timeout_ms": args.timeout_ms,
        "elapsed_ms": deadline.elapsed().as_millis(),
        "limits": {
            "max_entries": MAX_BATCH_ENTRIES,
            "max_input_bytes": MAX_BATCH_JSON_BYTES,
            "max_output_bytes": MAX_BATCH_OUTPUT_BYTES,
        }
    });
    if let Some(stopped) = stopped {
        body["stopped"] = stopped.to_value();
    }
    if serialized_size(&body) > MAX_BATCH_OUTPUT_BYTES {
        let disposition = if completed == 0 {
            DeliverySemantics::not_delivered()
        } else {
            DeliverySemantics::uncertain()
        };
        return Err(AdapterError::new(
            ErrorCode::Internal,
            "Batch response exceeded its output contract after final serialization",
        )
        .with_details(json!({
            "kind": "batch_output_limit",
            "completed_entries": completed,
            "max_output_bytes": MAX_BATCH_OUTPUT_BYTES,
        }))
        .with_disposition(disposition)
        .into());
    }
    Ok(body)
}

/// The session an entry would be refused for because it has ended, unless
/// the command runs in an ended session, as `session end` does.
fn session_ended_for(command: &PreparedCommand) -> Option<String> {
    if command.command.runs_in_an_ended_session() {
        return None;
    }
    let session_id = command.context.session_id()?;
    match agent_desktop_core::session::read_manifest(session_id) {
        Ok(Some(manifest)) if manifest.ended_at.is_some() => Some(session_id.to_owned()),
        _ => None,
    }
}

fn push_small_entry(results: &mut Vec<Value>, used: &mut usize, entry: Value) {
    *used = used.saturating_add(serialized_size(&entry).saturating_add(1));
    results.push(entry);
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
