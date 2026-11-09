use agent_desktop_core::{
    AdapterError, AppError, Deadline, DeliverySemantics, ErrorCode, PermissionReport,
    PlatformAdapter, context::CommandContext,
};
use serde_json::{Value, json};

use crate::{cli::Commands, cli_args::batch::BatchArgs, command_policy::CommandEffect};

use super::{
    batch_results::BatchResults,
    batch_stop::BatchStop,
    bounded_json::serialized_size,
    entry_errors::{baseline_error, batch_session_ended},
    entry_outcome::EntryOutcome,
    pending_seed::{PendingSeed, read_seed, seed_context},
    preparation::{MAX_BATCH_ENTRIES, MAX_BATCH_JSON_BYTES, prepare},
    result_entry::MAX_BATCH_OUTPUT_BYTES,
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
    let mut results = BatchResults::with_capacity(total);
    let mut seeds: Vec<Option<PendingSeed>> = (0..total).map(|_| None).collect();
    let mut stopped: Option<BatchStop> = None;

    'entries: for index in 0..total {
        if deadline.is_expired() {
            stopped = Some(results.deadline(index, &commands[index].name, args.timeout_ms));
            break;
        }

        let inherited_seed = (index == 0).then(|| {
            (
                context.event_baseline().cloned(),
                context.notification_seed().cloned(),
            )
        });
        let held = seeds[index].take().map(PendingSeed::into_read);

        if let Some(ended_session) = commands[index].ended_session() {
            let error = batch_session_ended(index, &commands[index].name, &ended_session);
            results.not_started(index, &commands[index].name, "session_ended", error);
            if args.stop_on_error {
                stopped = Some(BatchStop::StopOnError { index });
                break;
            }
            continue;
        }

        let seed = match held {
            Some((read_at, Err(error))) => {
                let error = baseline_error(&commands, index, read_at, index, error);
                results.not_started(
                    index,
                    &commands[index].name,
                    "pre_action_baseline_failed",
                    error,
                );
                if args.stop_on_error {
                    stopped = Some(BatchStop::StopOnError { index });
                    break;
                }
                continue;
            }
            Some((_, Ok((event, notification)))) => Some((event.map(Ok), notification)),
            None => inherited_seed,
        };

        for &wait_index in seed_plan.waits_read_before(index) {
            let read = read_seed(&commands[wait_index], adapter, deadline);
            if deadline.is_expired() {
                stopped = Some(results.deadline(index, &commands[index].name, args.timeout_ms));
                break 'entries;
            }
            match read {
                Err(error) if wait_index == index + 1 => {
                    let error = baseline_error(&commands, index, index, wait_index, error);
                    results.not_started(
                        index,
                        &commands[index].name,
                        "pre_action_baseline_failed",
                        error,
                    );
                    stopped = Some(BatchStop::BaselineFailed {
                        blocked_index: index,
                        blocked_command: commands[index].name.clone(),
                        wait_index,
                        wait_command: commands[wait_index].name.clone(),
                    });
                    break 'entries;
                }
                read => seeds[wait_index] = PendingSeed::held(index, read),
            }
        }

        let item_context = seed_context(
            seed,
            commands[index]
                .context
                .clone()
                .with_inherited_deadline(deadline),
        );
        let command = std::mem::replace(&mut commands[index].command, Commands::Version);
        let entry = super::result_entry::completed_entry(
            index,
            &commands[index].name,
            crate::dispatch::dispatch(command, adapter, permission_report, &item_context),
        );
        let batch_expired = deadline.is_expired();
        let outcome = EntryOutcome::of(index, &entry, batch_expired);
        let (entry, oversized) = super::result_entry::bounded_completed_entry(
            index,
            &commands[index].name,
            entry_effects[index],
            entry,
            results.bytes(),
        );
        results.push(entry);

        stopped = BatchStop::after_entry(
            &outcome.with_oversized(oversized),
            batch_expired && index + 1 < total,
            args.stop_on_error,
        );
        if stopped.is_some() {
            break;
        }
    }

    let completed = results.completed();
    let failed = results.failed();
    let mut body = json!({
        "results": results.into_entries(),
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
        "failed_entries": failed,
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

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "seed_read_tests.rs"]
mod seed_read_tests;

#[cfg(test)]
#[path = "session_end_tests.rs"]
mod session_end_tests;

#[cfg(test)]
#[path = "deadline_stop_tests.rs"]
mod deadline_stop_tests;
