use std::time::{Duration, Instant};

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ObservationRequest, ObservationRoot, ObservationSource,
    ObservedTree, SnapshotSurface,
};

use super::capabilities::headless_routes::HeadlessRoutes;
use super::element::ancestry;
use super::menu::observe_menu::observe_menu;
use super::observe::decorate::{Decorate, Plain};
use super::observe::projection::project_with;
use super::observe::readiness::{Readiness, has_web_document};
use super::observe::readiness_step::Step;
use super::placement::node_position::place_nodes;
use super::properties::read_purpose::ReadPurpose;
use super::walk::walker::{walk_structure, walk_tree_for};
use super::{AtspiElement, ObservedRead, Walk, WalkLimits, atspi_element};
use crate::system::process::toolkit_id::ToolkitId;
use crate::system::{SharedBus, process::proc_stat, shell, window::window_id};

/// A window of a stopped process cannot be read, so observing it is
/// `APP_UNRESPONSIVE` at once instead of a walk that waits out its budget.
fn refuse_stopped(
    window: &agent_desktop_core::WindowInfo,
    is_stopped: impl Fn(u32) -> bool,
) -> Result<(), AdapterError> {
    if is_stopped(window.pid.get()) {
        return Err(AdapterError::app_unresponsive(&window.app));
    }
    Ok(())
}

fn root_element(root: &ObservationRoot<'_>) -> Result<AtspiElement, AdapterError> {
    match root {
        ObservationRoot::Application { .. } => {
            Err(AdapterError::not_supported("application observation root"))
        }
        ObservationRoot::Window(window) if window.is_process_owner() => {
            Err(AdapterError::app_has_no_window(&window.app))
        }
        ObservationRoot::Window(window) => {
            refuse_stopped(window, proc_stat::is_stopped)?;
            let (bus, path) = window_id::decode(&window.id)?;
            let mut element = AtspiElement::new(bus, path, window.pid.get());
            element.set_ancestry(ancestry::of_root(&element));
            Ok(element)
        }
        ObservationRoot::Element { handle, .. } => atspi_element(handle).cloned(),
    }
}

fn window_gone(root: &ObservationRoot<'_>, error: AdapterError) -> AdapterError {
    let is_window = matches!(root, ObservationRoot::Window(_));
    let vanished = matches!(
        error.code,
        ErrorCode::AppNotFound | ErrorCode::ElementNotFound
    );
    if is_window && vanished {
        return AdapterError::new(
            ErrorCode::WindowNotFound,
            "The window is gone or its application left the accessibility bus",
        )
        .with_suggestion("Run 'list-windows' to see the windows that exist now")
        .with_platform_detail(error.message);
    }
    error
}

fn require_surface(surface: SnapshotSurface) -> Result<(), AdapterError> {
    match surface {
        SnapshotSurface::Window | SnapshotSurface::Sheet | SnapshotSurface::Alert => Ok(()),
        _ => Err(AdapterError::not_supported("observe_tree for this surface")),
    }
}

type Placer<'a> =
    dyn FnMut(&mut Walk<ObservedRead>) -> Result<Option<ToolkitId>, AdapterError> + 'a;

type Walker<'a> = dyn FnMut(
        &AtspiElement,
        &WalkLimits,
        Deadline,
        ReadPurpose,
    ) -> Result<Walk<ObservedRead>, AdapterError>
    + 'a;

/// Walks until the tree has stopped growing when the window of a web engine
/// is observed, otherwise once. A read that only resolves a saved ref is
/// never waited for: the ref was captured from a settled tree, and one
/// complete read of the live window is what it is judged by. The second value
/// is false when the wait ended on its budget with the tree still changing.
/// A later read that fails because the budget ran
/// out returns the walk before it; any other failure is the failure of the
/// observation.
fn settled_walk(
    root: &ObservationRoot<'_>,
    element: &AtspiElement,
    limits: &WalkLimits,
    request: &ObservationRequest,
    host: &mut SettleHost<'_>,
) -> Result<(Walk<ObservedRead>, bool), AdapterError> {
    if let Some(dialog) = nested_surface::discover(root, element, limits, request, host)? {
        return Ok(dialog);
    }
    let settles = !request.observation_mode.identity_only;
    let web_process =
        settles && matches!(root, ObservationRoot::Window(_)) && (host.is_web_process)(element)?;
    let mut tracker: Option<(Readiness, Instant)> = None;
    let mut previous: Option<Walk<ObservedRead>> = None;
    let mut sampling_deadline = request.deadline;
    let full = full_read(request);
    loop {
        let purpose = if full == ReadPurpose::Snapshot
            && tracker
                .as_ref()
                .is_some_and(|(readiness, _)| !readiness.needs_payload())
        {
            ReadPurpose::Sampling
        } else {
            full
        };
        let walk = match (host.walker)(element, limits, sampling_deadline, purpose) {
            Ok(walk) => walk,
            Err(error) if error.code == ErrorCode::Timeout && previous.is_some() => {
                return previous.map(|walk| (walk, false)).ok_or(error);
            }
            Err(error) => return Err(window_gone(root, error)),
        };
        let waits = settles
            && matches!(root, ObservationRoot::Window(_))
            && (web_process || has_web_document(&walk.root));
        if !walk.complete || !waits {
            return Ok(if purpose == ReadPurpose::Sampling {
                (
                    previous.ok_or_else(|| request.deadline.timeout_error())?,
                    false,
                )
            } else {
                (walk, true)
            });
        }
        if tracker.is_none() {
            sampling_deadline = request
                .deadline
                .capped(request.deadline.remaining() * 80 / 100);
        }
        let (settle, since) = tracker
            .get_or_insert_with(|| (Readiness::new(request.deadline.remaining()), Instant::now()));
        match settle.after_walk(
            walk.node_count,
            since.elapsed(),
            super::observe::readiness::ready(&walk.root),
        ) {
            Step::Settled => return Ok((walk, true)),
            Step::GiveUp => {
                return Ok((
                    if purpose == ReadPurpose::Sampling {
                        previous.unwrap_or(walk)
                    } else {
                        walk
                    },
                    false,
                ));
            }
            Step::Wait(gap) => {
                (host.pause)(gap);
                if purpose == full {
                    previous = Some(walk);
                }
            }
        }
    }
}

/// The walk's own read: identity evidence alone when the request only
/// resolves a saved ref, the full snapshot read otherwise.
fn full_read(request: &ObservationRequest) -> ReadPurpose {
    if request.observation_mode.identity_only {
        ReadPurpose::Count
    } else {
        ReadPurpose::Snapshot
    }
}

/// What the settle loop needs from outside the tree: the walk itself, the
/// structural scan that looks for a nested dialog and stops at web documents,
/// whether a process is a web engine or has stopped, and how to wait.
struct SettleHost<'a> {
    walker: &'a mut Walker<'a>,
    scan: &'a mut Walker<'a>,
    is_web_process: &'a dyn Fn(&AtspiElement) -> Result<bool, AdapterError>,
    stopped: &'a dyn Fn(u32) -> bool,
    pause: &'a mut dyn FnMut(Duration),
    place: &'a mut Placer<'a>,
}

#[cfg(test)]
fn never_scans(
    _: &AtspiElement,
    _: &WalkLimits,
    _: Deadline,
    _: ReadPurpose,
) -> Result<Walk<ObservedRead>, AdapterError> {
    Err(AdapterError::internal(
        "a window surface request never scans for a nested dialog",
    ))
}

/// A scan served by a test walker that ignores depth: a probe of the root
/// alone keeps the root and drops what it holds.
#[cfg(test)]
fn scan_of(
    limits: &WalkLimits,
    walk: impl FnOnce(&WalkLimits) -> Result<Walk<ObservedRead>, AdapterError>,
) -> Result<Walk<ObservedRead>, AdapterError> {
    let whole = WalkLimits {
        max_raw_depth: limits
            .max_raw_depth
            .max(WalkLimits::default().max_raw_depth),
        ..*limits
    };
    let mut scanned = walk(&whole)?;
    if limits.max_raw_depth == 0 {
        scanned.root.children.clear();
    }
    Ok(scanned)
}

#[cfg(test)]
fn observe_with(
    root: ObservationRoot<'_>,
    request: &ObservationRequest,
    host: SettleHost<'_>,
) -> Result<ObservedTree, AdapterError> {
    observe_decorated(root, request, host, &mut Plain, || None)
}

fn observe_decorated(
    root: ObservationRoot<'_>,
    request: &ObservationRequest,
    mut host: SettleHost<'_>,
    decorate: &mut dyn Decorate,
    read_displays: impl FnOnce() -> Option<Vec<super::Extents>>,
) -> Result<ObservedTree, AdapterError> {
    let request = (*request).validate()?;
    if let ObservationRoot::Window(window) = &root {
        require_surface(request.surface)?;
        if (host.stopped)(window.pid.get()) {
            return Err(AdapterError::app_unresponsive(&window.app));
        }
    }
    let element = root_element(&root)?;
    let limits = WalkLimits {
        root_in_web: element.in_web_document(),
        ..WalkLimits::for_request(&request)
    };
    let started = Instant::now();
    let (mut walk, settled) = settled_walk(&root, &element, &limits, &request, &mut host)?;
    let toolkit = if request.observation_mode.identity_only {
        None
    } else {
        (host.place)(&mut walk)?
    };
    let routes = HeadlessRoutes::of_walk(&walk, toolkit);
    let complete = walk.complete && settled;
    let displays = read_displays();
    let (subtree, mut stats) = project_with(walk, &limits, decorate, displays.as_deref(), &routes);
    stats.reads.health.deadline_exhausted += u64::from(!complete && request.deadline.is_expired());
    stats.elapsed_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    ObservedTree::from_roots(
        vec![subtree],
        ObservationSource::from_root(&root, request.surface),
        stats,
        complete,
    )
}

/// Observes the tree under a window or an element: the walk, then the
/// projection into core's observed tree. An observation the deadline or a
/// budget cut short is a success that reports `structurally_complete` false.
pub(crate) fn observe_tree(
    bus: &SharedBus,
    root: ObservationRoot<'_>,
    request: &ObservationRequest,
) -> Result<ObservedTree, AdapterError> {
    if let ObservationRoot::Window(window) = &root {
        if request.surface == SnapshotSurface::Menu {
            return observe_menu(bus, window, request);
        }
        if shell::kinds::kind_of(request.surface).is_some() {
            return shell::observe_shell(bus, window, request);
        }
    }
    if let ObservationRoot::Element { handle, .. } = &root {
        let element = atspi_element(handle)?;
        if element.is_menu_target() {
            let context = super::menu::menu_context::for_element(bus, element, request.deadline)?
                .ok_or_else(AdapterError::menu_closed)?;
            return observe_tree_with(
                bus,
                root,
                request,
                &mut super::menu::menu_identity::MenuIdentity::new(context),
            );
        }
    }
    observe_tree_with(bus, root, request, &mut Plain)
}

/// `observe_tree` for a root that is not routed by surface, with a hook that
/// may decorate the projected nodes and append its own.
pub(crate) fn observe_tree_with(
    bus: &SharedBus,
    root: ObservationRoot<'_>,
    request: &ObservationRequest,
    decorate: &mut dyn Decorate,
) -> Result<ObservedTree, AdapterError> {
    let mut walker = |element: &AtspiElement, limits: &WalkLimits, deadline: Deadline, purpose| {
        walk_tree_for(bus, element, limits, deadline, purpose)
    };
    let mut scan = |element: &AtspiElement, limits: &WalkLimits, deadline: Deadline, purpose| {
        walk_structure(bus, element, limits, deadline, purpose)
    };
    let mut pause = std::thread::sleep;
    use crate::system::x11::{self, MonitorReader, MonitorSource};
    let displays = x11::start(|| MonitorReader.read());
    observe_decorated(
        root,
        request,
        SettleHost {
            walker: &mut walker,
            scan: &mut scan,
            place: &mut |walk| place_nodes(bus, walk, request.deadline),
            is_web_process: &|element| {
                let connection = bus.get(request.deadline)?;
                crate::system::within(request.deadline, async {
                    Ok(super::properties::toolkit_trust::read_web_family(
                        connection.inner(),
                        element,
                    )
                    .await?
                    .is_some())
                })
            },
            stopped: &crate::system::process::proc_stat::is_stopped,
            pause: &mut pause,
        },
        decorate,
        || match displays.finish(request.deadline.capped(std::time::Duration::ZERO)) {
            Some(Ok(monitors)) => {
                Some(monitors.into_iter().map(|monitor| monitor.bounds).collect())
            }
            _ => None,
        },
    )
}

pub(crate) mod decorate;
mod nested_surface;
pub(super) mod node_evidence;
pub(super) mod observation;
pub(super) mod observed_read;
pub(crate) mod projection;
pub(super) mod readiness;
pub(super) mod readiness_step;
#[cfg(test)]
#[path = "observe_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "observe_routes_tests.rs"]
mod routes_tests;

#[cfg(test)]
#[path = "observe_nested_tests.rs"]
mod nested_tests;

#[cfg(test)]
#[path = "observe_settle_tests.rs"]
mod settle_tests;

#[cfg(test)]
#[path = "observe_sampling_tests.rs"]
mod sampling_tests;
