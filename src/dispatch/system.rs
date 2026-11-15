use agent_desktop_core::{
    AppError, PermissionReport, PlatformAdapter,
    commands::{
        permissions as permissions_command, skills as skills_command, wait as wait_command,
        wait_surface::SurfaceWait,
    },
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::{
    skills::{SkillsAction, SkillsArgs},
    system::{PermissionsArgs, WaitArgs},
};

pub(super) fn wait(
    args: WaitArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    wait_command::execute(core_wait_args(&args)?, adapter, context)
}

/// The core form of a `wait` entry, shared by dispatch and by the batch
/// pre-action seed so both validate the same arguments.
pub(crate) fn core_wait_args(args: &WaitArgs) -> Result<wait_command::WaitArgs, AppError> {
    Ok(wait_command::WaitArgs {
        mode: wait_command::WaitModeArgs {
            ms: args.mode.ms,
            element: args.mode.element.clone(),
            window: args.mode.window.clone(),
            text: args.mode.text.clone(),
            surface: SurfaceWait::with_download(
                SurfaceWait::from_flags(
                    args.surface.menu,
                    args.surface.menu_closed,
                    args.surface.notification,
                )?,
                args.surface.download,
            )?,
            event: args.event.event.clone(),
            window_id: args.event.window_id.clone(),
        },
        predicate: wait_command::WaitPredicateArgs {
            predicate: args.predicate.predicate.clone(),
            value: args.predicate.value.clone(),
            action: args.predicate.action.clone(),
            count: args.predicate.count,
        },
        timeout_ms: args.timeout,
        app: args.app.clone(),
    })
}

pub(super) fn permissions(
    args: PermissionsArgs,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
) -> Result<Value, AppError> {
    permissions_command::execute_with_report(
        permissions_command::PermissionsArgs {
            request: args.request,
            guide: args.guide,
        },
        adapter,
        permission_report,
    )
}

pub(crate) fn skills(args: SkillsArgs) -> Result<Value, AppError> {
    match args.action.unwrap_or(SkillsAction::List) {
        SkillsAction::List => skills_command::list(),
        SkillsAction::Path => skills_command::path(),
        SkillsAction::Get(get) => skills_command::get(skills_command::GetArgs {
            name: get.name,
            full: get.full,
            reference: get.reference,
        }),
    }
}
