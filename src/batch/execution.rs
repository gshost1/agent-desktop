use agent_desktop_core::{
    AdapterError, AppError, Deadline, DeliverySemantics, ErrorCode, NotificationSeed,
    PermissionReport, PlatformAdapter, SignalBaseline, SignalFilter,
    commands::wait_notification as notification_wait, context::CommandContext,
};
use serde_json::{Value, json};

use crate::{cli::Commands, cli_args::batch::BatchArgs, command_policy::CommandEffect};

use super::{
    bounded_json::serialized_size,
    preparation::{MAX_BATCH_ENTRIES, MAX_BATCH_JSON_BYTES, PreparedCommand, prepare},
    result_entry::{MAX_BATCH_OUTPUT_BYTES, not_started_entry},
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
    let mut results = Vec::with_capacity(total);
    let mut results_bytes = 0;
    let mut completed = 0;
    let mut pending_baseline: Option<Result<SignalBaseline, AdapterError>> =
        context.event_baseline().cloned();
    let mut pending_notification = context.notification_seed().cloned();
    let mut stopped = None;

    for index in 0..total {
        if deadline.is_expired() {
            let error = batch_timeout(index, &commands[index].name, args.timeout_ms);
            push_small_entry(
                &mut results,
                &mut results_bytes,
                not_started_entry(index, &commands[index].name, "deadline", error),
            );
            stopped = Some(json!({ "reason": "deadline", "index": index }));
            break;
        }

        let current_baseline = pending_baseline.take();
        let current_notification = pending_notification.take();

        if let Some(ended_session) = session_ended_for(&commands[index]) {
            let error = batch_session_ended(index, &commands[index].name, &ended_session);
            push_small_entry(
                &mut results,
                &mut results_bytes,
                not_started_entry(index, &commands[index].name, "session_ended", error),
            );
            if args.stop_on_error {
                stopped = Some(json!({ "reason": "stop_on_error", "index": index }));
                break;
            }
            continue;
        }

        match pre_action_seed(commands.get(index + 1), adapter, deadline) {
            Ok((event, notification)) => {
                pending_baseline = event.map(Ok);
                pending_notification = notification;
            }
            Err(error) => {
                let wait_index = index + 1;
                let wait_command = &commands[wait_index].name;
                let error = baseline_error(
                    index,
                    &commands[index].name,
                    wait_index,
                    wait_command,
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
                stopped = Some(json!({
                    "reason": "pre_action_baseline_failed",
                    "blocked_index": index,
                    "blocked_command": commands[index].name,
                    "wait_index": wait_index,
                    "wait_command": wait_command,
                }));
                break;
            }
        }

        let item_context = commands[index]
            .context
            .clone()
            .with_inherited_deadline(deadline)
            .with_event_baseline(current_baseline)
            .with_notification_seed(current_notification);
        let command = std::mem::replace(&mut commands[index].command, Commands::Version);
        let effect = entry_effects[index];
        let entry = super::result_entry::completed_entry(
            index,
            &commands[index].name,
            crate::dispatch::dispatch(command, adapter, permission_report, &item_context),
        );
        let failed = entry["ok"] != true;
        let (entry, oversized) = super::result_entry::bounded_completed_entry(
            index,
            &commands[index].name,
            effect,
            entry,
            results_bytes,
        );
        completed += usize::from(entry["execution"] == "completed");
        push_small_entry(&mut results, &mut results_bytes, entry);

        if oversized {
            stopped = Some(json!({ "reason": "output_limit", "index": index }));
            break;
        }
        if failed && args.stop_on_error {
            stopped = Some(json!({ "reason": "stop_on_error", "index": index }));
            break;
        }
        if deadline.is_expired() && index + 1 < total {
            stopped = Some(json!({ "reason": "deadline", "after_index": index }));
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
        body["stopped"] = stopped;
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

type PreActionSeed = (Option<SignalBaseline>, Option<NotificationSeed>);

/// Reads what the following wait entry diffs against before the current entry
/// runs, so a window, surface or notification the current entry produces is
/// reported as new instead of being absorbed into the wait's own first read.
/// Only an event baseline failure is an error: a notification seed that cannot
/// be read falls back to the wait's own first list and never blocks the entry.
fn pre_action_seed(
    next: Option<&PreparedCommand>,
    adapter: &dyn PlatformAdapter,
    deadline: Deadline,
) -> Result<PreActionSeed, AdapterError> {
    let Some(next) = next else {
        return Ok((None, None));
    };
    let Commands::Wait(args) = &next.command else {
        return Ok((None, None));
    };
    if args.mode.notification {
        let Ok(core_args) = crate::dispatch::system::core_wait_args(args) else {
            return Ok((None, None));
        };
        let context = next.context.clone().with_inherited_deadline(deadline);
        let seed = notification_wait::pre_action_seed(&core_args, deadline, adapter, &context);
        return Ok((None, seed));
    }
    if args.event.event.is_some() {
        let filter = SignalFilter {
            app: args.app.clone(),
            process: None,
        };
        return Ok((
            Some(adapter.capture_signal_baseline(&filter, deadline)?),
            None,
        ));
    }
    Ok((None, None))
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

fn session_ended_for(command: &PreparedCommand) -> Option<String> {
    let session_id = command.context.session_id()?;
    match agent_desktop_core::session::read_manifest(session_id) {
        Ok(Some(manifest)) if manifest.ended_at.is_some() => Some(session_id.to_owned()),
        _ => None,
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
