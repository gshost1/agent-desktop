use agent_desktop_core::{
    AppError, PlatformAdapter,
    commands::{
        helpers, scroll as scroll_command, select as select_command,
        set_value as set_value_command, type_text as type_text_command,
    },
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::{
    RefArgs,
    actions::{ScrollArgs, SelectArgs, SetValueArgs, TypeArgs},
};
use crate::dispatch::parse::parse_direction;

pub(super) fn type_text(
    args: TypeArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let timeout_ms = match args.interval_ms {
        Some(_) => args.timeout_ms,
        None => helpers::normalize_action_timeout_ms(args.effective_timeout_ms()),
    };
    let text = args
        .text
        .ok_or_else(|| AppError::invalid_input("type text is missing"))?;
    type_text_command::execute(
        type_text_command::TypeArgs {
            ref_id: args.ref_id,
            text,
            timeout_ms,
            interval_ms: args.interval_ms,
        },
        adapter,
        context,
    )
}

pub(super) fn set_value(
    args: SetValueArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let value = args
        .value
        .ok_or_else(|| AppError::invalid_input("set-value input is missing"))?;
    set_value_command::execute(
        set_value_command::SetValueArgs {
            ref_id: args.ref_id,
            value,
            timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
        },
        adapter,
        context,
    )
}

pub(super) fn select(
    args: SelectArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    select_command::execute(
        select_command::SelectArgs {
            ref_id: args.ref_id,
            value: args.value,
            timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
        },
        adapter,
        context,
    )
}

pub(super) fn scroll(
    args: ScrollArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    scroll_command::execute(
        scroll_command::ScrollArgs {
            ref_id: args.ref_id,
            direction: parse_direction(&args.direction)?,
            amount: args.amount,
            timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
        },
        adapter,
        context,
    )
}

pub(super) fn ref_args(args: RefArgs) -> helpers::RefArgs {
    helpers::RefArgs {
        ref_id: args.ref_id,
        timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
    }
}
