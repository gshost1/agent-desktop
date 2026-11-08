use agent_desktop_core::{
    AdapterError, AppError, Deadline, DeliverySemantics, ErrorCode, PermissionReport,
    PlatformAdapter, context::CommandContext,
};
use serde_json::{Value, json};

use crate::{
    cli::Commands,
    cli_args::{batch::BatchArgs, session::SessionAction},
    command_policy::CommandEffect,
};

use super::{
    batch_stop::BatchStop,
    bounded_json::serialized_size,
    pending_seed::PendingSeed,
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

        let current_seed = PendingSeed::take_for(&mut pending_seed, index);

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

        if let Some(wait_index) = seed_plan.wait_read_before(index) {
            match PendingSeed::capture(wait_index, &commands[wait_index], adapter, deadline) {
                Ok(seed) => pending_seed = seed,
                Err(error) => {
                    let blocked_command = commands[index].name.clone();
                    let wait_command = commands[wait_index].name.clone();
                    let error =
                        baseline_error(index, &blocked_command, wait_index, &wait_command, error);
                    push_small_entry(
                        &mut results,
                        &mut results_bytes,
                        not_started_entry(
                            index,
                            &blocked_command,
                            "pre_action_baseline_failed",
                            error,
                        ),
                    );
                    stopped = Some(BatchStop::BaselineFailed {
                        blocked_index: index,
                        blocked_command,
                        wait_index,
                        wait_command,
                    });
                    break;
                }
            }
        }

        let item_context = PendingSeed::seed_context(
            current_seed,
            commands[index]
                .context
                .clone()
                .with_inherited_deadline(deadline),
        );
        let command = std::mem::replace(&mut commands[index].command, Commands::Version);
        let effect = entry_effects[index];
        let entry = super::result_entry::completed_entry(
            index,
            &commands[index].name,
            crate::dispatch::dispatch(command, adapter, permission_report, &item_context),
        );
        let failure = (entry["ok"] != true).then(|| {
            entry["error"]["code"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        });
        let (entry, oversized) = super::result_entry::bounded_completed_entry(
            index,
            &commands[index].name,
            effect,
            entry,
            results_bytes,
        );
        completed += usize::from(entry["execution"] == "completed");
        push_small_entry(&mut results, &mut results_bytes, entry);

        stopped = BatchStop::after_entry(
            index,
            failure.as_deref(),
            oversized,
            deadline.is_expired() && index + 1 < total,
            args.stop_on_error,
        );
        if stopped.is_some() {
            break;
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

fn baseline_error(
    blocked_index: usize,
    blocked_command: &str,
    wait_index: usize,
    wait_command: &str,
    mut source: AdapterError,
) -> AppError {
    let cause_details = source.details.take();
    source.message = format!(
        "Batch entry {blocked_index} ('{blocked_command}') was not started because the baseline for following wait entry {wait_index} ('{wait_command}') failed: {}",
        source.message
    );
    let mut details = json!({
        "kind": "pre_action_baseline_failed",
        "blocked_index": blocked_index,
        "blocked_command": blocked_command,
        "wait_index": wait_index,
        "wait_command": wait_command,
    });
    if let Some(cause_details) = cause_details {
        details["cause_details"] = cause_details;
    }
    source.details = Some(details);
    source.disposition = DeliverySemantics::not_delivered();
    source.into()
}

/// The session an entry would be refused for because it has ended. A
/// `session end` entry is never refused: teardown is repeatable on an ended
/// session, as it is outside a batch.
fn session_ended_for(command: &PreparedCommand) -> Option<String> {
    if runs_in_an_ended_session(&command.command) {
        return None;
    }
    let session_id = command.context.session_id()?;
    match agent_desktop_core::session::read_manifest(session_id) {
        Ok(Some(manifest)) if manifest.ended_at.is_some() => Some(session_id.to_owned()),
        _ => None,
    }
}

fn runs_in_an_ended_session(command: &Commands) -> bool {
    match command {
        Commands::Session(args) => matches!(args.action, SessionAction::End(_)),
        _ => false,
    }
}

fn batch_session_ended(index: usize, command: &str, session_id: &str) -> AppError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!(
            "Batch entry {index} ('{command}') was not started because session '{session_id}' has ended"
        ),
    )
    .with_suggestion("Re-run the entry outside the ended session or start a new session")
    .with_details(json!({
        "kind": "batch_session_ended",
        "batch_index": index,
        "batch_command": command,
        "session_id": session_id,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
    .into()
}

fn batch_timeout(index: usize, command: &str, timeout_ms: u64) -> AppError {
    AdapterError::timeout("Batch deadline elapsed before the entry started")
        .with_details(json!({
            "kind": "batch_deadline",
            "batch_index": index,
            "batch_command": command,
            "timeout_ms": timeout_ms,
        }))
        .with_disposition(DeliverySemantics::not_delivered())
        .into()
}

fn push_small_entry(results: &mut Vec<Value>, used: &mut usize, entry: Value) {
    *used = used.saturating_add(serialized_size(&entry).saturating_add(1));
    results.push(entry);
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
