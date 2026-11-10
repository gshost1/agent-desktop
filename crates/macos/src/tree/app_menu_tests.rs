use super::super::menu_search::surface_limit_error;
use super::*;
use std::time::Duration;

#[test]
fn only_complete_menu_root_absence_gets_a_typed_surface_error() {
    for (surface, message) in [
        (SnapshotSurface::Menu, "No open menu"),
        (SnapshotSurface::Menu, "No open context menu"),
        (SnapshotSurface::Menubar, "No menu bar found"),
    ] {
        let error = require_root::<u8>(Ok(None), surface, message).unwrap_err();
        assert_eq!(error.code, ErrorCode::ElementNotFound);
        assert_eq!(error.message, message);
        assert_eq!(
            error.suggestion.as_deref(),
            Some(if surface == SnapshotSurface::Menu {
                "Open the menu, then take a fresh snapshot with --surface menu"
            } else {
                "Ensure the application exposes the menubar surface, then take a fresh snapshot with --surface menubar"
            })
        );
        assert_eq!(
            error.details.unwrap(),
            serde_json::json!({
                "kind": "surface_absent",
                "complete": true,
                "surface": surface.as_str(),
            })
        );
        assert_eq!(require_root(Ok(Some(7)), surface, message).unwrap(), 7);
        let invalidated = AdapterError::new(ErrorCode::AppUnresponsive, "invalidated root")
            .with_details(serde_json::json!({
                "ax_error": accessibility_sys::kAXErrorInvalidUIElement,
                "complete": false,
            }));
        let error = require_root::<u8>(Err(invalidated), surface, message).unwrap_err();
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(error.message, "invalidated root");
        assert_eq!(
            error.details.unwrap(),
            serde_json::json!({
                "ax_error": accessibility_sys::kAXErrorInvalidUIElement,
                "complete": false,
            })
        );
    }
}

fn current_identity() -> crate::system::process::process_identity::ProcessIdentity {
    crate::system::process::process_identity::ProcessIdentity::capture(
        i32::try_from(std::process::id()).unwrap(),
    )
    .unwrap()
    .unwrap()
}

#[test]
fn menu_observation_retries_native_transience_but_not_deterministic_incompleteness() {
    let mut reads = 0;
    let result = observe(
        current_identity(),
        Instant::now() + Duration::from_secs(1),
        || {
            reads += 1;
            if reads == 1 {
                Err(surface_read::map_error(
                    "AXRole",
                    accessibility_sys::kAXErrorCannotComplete,
                ))
            } else {
                Ok("fresh menu")
            }
        },
    );
    assert_eq!(result.unwrap(), "fresh menu");
    assert_eq!(reads, 2);
    let mut reads = 0;
    let error = observe(
        current_identity(),
        Instant::now() + Duration::from_secs(1),
        || {
            reads += 1;
            match reads {
                1 | 3 => Err(surface_read::map_error(
                    "AXRole",
                    accessibility_sys::kAXErrorInvalidUIElement,
                )),
                2 => Err(surface_read::map_error(
                    "AXRole",
                    accessibility_sys::kAXErrorCannotComplete,
                )),
                _ => Ok("must not reacquire again"),
            }
        },
    )
    .unwrap_err();
    assert_eq!(reads, 3);
    assert!(!error.permits_retry_by_default());
    assert_eq!(error.details.unwrap()["menu_root_reacquired"], true);
    for expected in [surface_limit_error(), invalid_root_error()] {
        let mut attempts = 0;
        let error = observe::<()>(
            current_identity(),
            Instant::now() + Duration::from_secs(1),
            || {
                attempts += 1;
                assert_eq!(attempts, 1);
                Err(expected.clone())
            },
        )
        .unwrap_err();
        assert_eq!(error.details, expected.details);
        assert!(!error.permits_retry_by_default());
    }
    let error = observe::<()>(current_identity(), Instant::now(), || {
        panic!("expired observation must not read a menu")
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn terminal_menu_errors_are_not_retried() {
    for (code, details) in [
        (
            ErrorCode::PermDenied,
            serde_json::json!({ "complete": false }),
        ),
        (
            ErrorCode::StaleRef,
            serde_json::json!({ "complete": false }),
        ),
        (
            ErrorCode::AmbiguousTarget,
            serde_json::json!({ "complete": true }),
        ),
        (
            ErrorCode::ElementNotFound,
            serde_json::json!({ "complete": true }),
        ),
        (
            ErrorCode::AppUnresponsive,
            serde_json::json!({ "complete": false, "retryable": false }),
        ),
    ] {
        let mut reads = 0;
        let error = observe::<()>(
            current_identity(),
            Instant::now() + Duration::from_secs(1),
            || {
                reads += 1;
                assert_eq!(reads, 1);
                Err(AdapterError::new(code.clone(), "terminal").with_details(details.clone()))
            },
        )
        .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(reads, 1);
    }
}

#[test]
fn process_exit_rejects_menu_reads_before_and_after_observation() {
    for phase in ["before", "success", "incomplete", "absent"] {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let identity = crate::system::process::process_identity::ProcessIdentity::capture(
            i32::try_from(child.id()).unwrap(),
        )
        .unwrap()
        .unwrap();
        if phase == "before" {
            child.kill().unwrap();
            child.wait().unwrap();
        }
        let mut reads = 0;
        let error = observe(identity, Instant::now() + Duration::from_secs(1), || {
            reads += 1;
            assert_ne!(phase, "before");
            child.kill().unwrap();
            child.wait().unwrap();
            if phase == "incomplete" {
                Err(surface_limit_error())
            } else if phase == "absent" {
                require_root(Ok(None), SnapshotSurface::Menu, "No open menu")
            } else {
                Ok("read from exited process")
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleRef);
        assert_eq!(reads, usize::from(phase != "before"));
    }
}

#[test]
fn open_menu_ids_follow_unfiltered_menubar_provenance() {
    let discover = |selected: &[u8]| {
        open_menubar_menus_with(
            &0_u8,
            |node| {
                Ok(match node {
                    0 => vec![10, 20, 30],
                    10 => vec![11, 12],
                    20 => vec![21, 22],
                    30 => vec![31, 32],
                    _ => unreachable!(),
                })
            },
            |node| Ok(selected.contains(node)),
            |node| Ok(node % 10 == 2),
        )
        .unwrap()
    };
    let file = discover(&[10]);
    let edit = discover(&[20]);
    let both = discover(&[10, 20]);
    assert_eq!(file, [("app/menubar/children/0/children/1".into(), 12)]);
    assert_eq!(edit, [("app/menubar/children/1/children/1".into(), 22)]);
    assert_eq!(both, [file[0].clone(), edit[0].clone()]);
}

#[test]
fn incomplete_selected_menu_children_cannot_produce_an_empty_or_partial_inventory() {
    let error = open_menubar_menus_with(
        &0_u8,
        |node| match node {
            0 => Ok(vec![1, 2]),
            1 => Ok(vec![3]),
            _ => Err(surface_limit_error()),
        },
        |_| Ok(true),
        |_| Ok(true),
    )
    .unwrap_err();
    assert_eq!(error.details.unwrap()["complete"], false);
}

#[test]
fn invalidated_menu_roots_are_reacquired_once_and_permission_errors_are_terminal() {
    use accessibility_sys::{kAXErrorAPIDisabled, kAXErrorInvalidUIElement};
    for (ax_error, failures) in [
        (kAXErrorInvalidUIElement, 1),
        (kAXErrorInvalidUIElement, 2),
        (kAXErrorAPIDisabled, 1),
    ] {
        let mut reads = 0;
        let result = reacquire_once(
            Instant::now() + std::time::Duration::from_secs(1),
            &mut false,
            || {
                reads += 1;
                if reads <= failures {
                    Err(AdapterError::new(ErrorCode::PermDenied, "read failed")
                        .with_details(serde_json::json!({ "ax_error": ax_error })))
                } else {
                    Ok("fresh root")
                }
            },
        );
        if ax_error == kAXErrorAPIDisabled {
            assert_eq!(reads, 1);
            assert_eq!(result.unwrap_err().code, ErrorCode::PermDenied);
        } else {
            assert_eq!(reads, 2);
            if failures == 1 {
                assert_eq!(result.unwrap(), "fresh root");
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.code, ErrorCode::AppUnresponsive);
                let details = error.details.unwrap();
                assert_eq!(details["complete"], false);
                assert_eq!(details["menu_root_reacquired"], true);
                assert_eq!(details["retryable"], false);
                assert_eq!(details["ax_error"], kAXErrorInvalidUIElement);
            }
        }
    }
}
