use agent_desktop_core::{AdapterError, AppError, DeliverySemantics, ErrorCode};

use crate::command_policy::CommandEffect;

/// Settles the delivery of an error that carries none, by what the command
/// can change. A read-only or local-write command sends nothing to an
/// application, so its failure is `not_delivered`; one that has no
/// disposition slot (`Io`, `Json`, `Internal`) becomes an adapter error that
/// keeps its code and message. A command that sends input keeps `unknown`,
/// because a mutation that timed out may already have landed, except for an
/// argument refusal raised before any preparatory focus or scroll ran.
/// An unreadable-part refusal keeps its non-safe retry disposition: more
/// time cannot establish the requested count or pick.
pub(crate) fn settle_unknown_delivery(error: AppError, effect: CommandEffect) -> AppError {
    match error {
        AppError::Adapter(source) => settle(source, effect).into(),
        other if !effect.sends_input() => settle(other.into_adapter_error(), effect).into(),
        other => other,
    }
}

/// A failure raised before dispatch sent nothing. A permission denial keeps
/// `PERM_DENIED`; any other error that is not an adapter error is `INTERNAL`.
pub(crate) fn pre_dispatch_error(error: AppError) -> AppError {
    error
        .into_adapter_error()
        .with_disposition(DeliverySemantics::not_delivered())
        .into()
}

fn settle(source: AdapterError, effect: CommandEffect) -> AdapterError {
    if !source.permits_retry_by_default()
        && source
            .details
            .as_ref()
            .is_some_and(|details| details["kind"] == "locator_unreadable_part")
    {
        return source;
    }
    if source.disposition == DeliverySemantics::unknown() && sent_nothing(&source, effect) {
        source.with_disposition(DeliverySemantics::not_delivered())
    } else {
        source
    }
}

fn sent_nothing(source: &AdapterError, effect: CommandEffect) -> bool {
    !effect.sends_input()
        || (source.code == ErrorCode::InvalidArgs
            && !source.moved_focus()
            && !source.changed_view())
}
