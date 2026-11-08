use super::{CommandContext, result_disposition, trace_error_with_disposition};
use crate::{AppError, DeliverySemantics, wait::deadline::DeadlineScope};
use serde_json::{Value, json};
use std::cell::Cell;
use std::time::Instant;

/// Emits `command.start` on construction and `command.end` on `complete`.
/// The `Drop` guard emits a fallback `command.end` for a scope abandoned
/// without `complete` (normal early return, or a panic under the unwind
/// test profile). It cannot fire on a genuine panic in the release binary:
/// `[profile.release]` uses `panic = "abort"`, which terminates without
/// unwinding `Drop`. A panicked release command therefore leaves an
/// unpaired `command.start`, which the trace reader tolerates as an
/// `unpaired_command` warning rather than a lost record.
/// `command.end` carries `command_seq`, the `seq` of this scope's own
/// `command.start`, so the reader pairs overlapping scopes of one command.
pub struct CommandScope<'a> {
    context: &'a CommandContext,
    command: &'static str,
    success_disposition: DeliverySemantics,
    started: Instant,
    finished: Cell<bool>,
    start_seq: Option<u64>,
    _deadline_scope: DeadlineScope,
}

impl<'a> CommandScope<'a> {
    pub(super) fn begin(
        context: &'a CommandContext,
        command: &'static str,
        success_disposition: DeliverySemantics,
        deadline_scope: DeadlineScope,
    ) -> Result<Self, AppError> {
        let start_seq = context
            .trace
            .emit_recorded(
                "command.start",
                context.session_id(),
                || json!({ "command": command }),
            )
            .map_err(|error| {
                trace_error_with_disposition(error, DeliverySemantics::not_delivered())
            })?;
        Ok(Self {
            context,
            command,
            success_disposition,
            started: Instant::now(),
            finished: Cell::new(false),
            start_seq,
            _deadline_scope: deadline_scope,
        })
    }

    pub fn complete(self, result: &Result<Value, AppError>) -> Result<(), AppError> {
        self.finished.set(true);
        let emitted = match result {
            Ok(_) => self.emit_end(true, None),
            Err(err) => self.emit_end(false, Some(err.code())),
        };
        emitted.map_err(|error| {
            trace_error_with_disposition(
                error,
                result_disposition(result, self.success_disposition),
            )
        })
    }

    fn emit_end(&self, ok: bool, code: Option<&str>) -> Result<(), AppError> {
        let mut fields = json!({
            "command": self.command,
            "ok": ok,
            "duration_ms": self.started.elapsed().as_millis(),
        });
        if let Some(code) = code {
            fields["code"] = json!(code);
        }
        if let Some(seq) = self.start_seq {
            fields["command_seq"] = json!(seq);
        }
        self.context.trace("command.end", fields)
    }
}

impl Drop for CommandScope<'_> {
    fn drop(&mut self) {
        if self.finished.get() {
            return;
        }
        let _ = self.emit_end(false, Some("INTERNAL"));
    }
}
