use serde_json::Value;

use crate::{
    AppError,
    adapter::PlatformAdapter,
    commands::snapshot::{SnapshotArgs, format_snapshot_fields, tree_options},
    context::CommandContext,
    refs::{refs_store::RefStore, validate_snapshot_id},
    snapshot,
    snapshot::diff::{diff_baseline, diff_match, diff_presentation::DiffPresentation},
};

/// `snapshot --since <snapshot_id>`: takes a normal full snapshot, with its
/// own snapshot id and saved refs, and returns only what differs from the
/// earlier snapshot `since` of the same session in `data.diff` instead of the
/// whole tree. When the two cannot be compared the whole tree is returned.
pub fn execute_since(
    args: SnapshotArgs,
    since: &str,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    if args.root_ref.is_some() {
        return Err(AppError::invalid_input_with_suggestion(
            "--since cannot be combined with --root",
            "Run snapshot --since without --root; a diff compares whole snapshots.",
        ));
    }
    if context.wait_selector().is_some() {
        return Err(AppError::invalid_input_with_suggestion(
            "--since cannot be combined with --wait-for or --wait-for-gone",
            "Wait with the wait command first, then run snapshot --since.",
        ));
    }
    validate_snapshot_id(since)?;
    adapter.check_surface_support(args.surface)?;
    let store = RefStore::for_session(context.session_id())?;
    let baseline = diff_baseline::load_baseline(&store, since)?;
    let opts = tree_options(&args);
    let result = snapshot::run_with_context(
        adapter,
        &opts,
        &snapshot::SnapshotTarget {
            app_name: args.app.as_deref(),
            window_id: args.window_id.as_deref(),
        },
        context,
        args.timeout_ms
            .unwrap_or(snapshot::DEFAULT_SNAPSHOT_TIMEOUT_MS),
    )?;
    let diff = diff_match::compare(since, &baseline, &result, DiffPresentation::of(&opts));
    let mut body = format_snapshot_fields(&result, None, None)?;
    if diff.is_comparable()
        && let Some(fields) = body.as_object_mut()
    {
        fields.remove("tree");
    }
    body["diff"] = diff.to_json()?;
    Ok(body)
}

#[cfg(test)]
#[path = "snapshot_since_tests.rs"]
mod tests;
