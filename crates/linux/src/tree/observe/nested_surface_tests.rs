use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ObservationRequest, ObservationRoot, SnapshotSurface,
};

use super::super::tests::{request, window};
use super::super::{SettleHost, observe_with};
use crate::system::window::window_id;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::vocabulary::role_numbers::{DIALOG, DOCUMENT_WEB, FRAME, PANEL};
use crate::tree::vocabulary::state_bits::{MODAL, SHOWING};
use crate::tree::walk::walker::{walk_levels, walk_structure_levels};
use crate::tree::walk::wrapper::classify;
use crate::tree::{AtspiElement, Expansion, Field, ObservedRead, Walk, WalkLimits};

const PAGE_NODES: u32 = 40;

#[derive(Clone, Copy)]
enum Window {
    Plain,
    HoldingADialog,
    ItselfTheSheet,
    HoldingAPage,
}

fn node(key: u32, shape: Window) -> Expansion<ObservedRead, u32> {
    let showing = Field::Present(words(&[SHOWING]));
    if key == 0 {
        let mut frame = match shape {
            Window::ItselfTheSheet => read_of(DIALOG, "w", &[]),
            _ => read_of(FRAME, "w", &[]),
        };
        frame.node.traits.states = match shape {
            Window::ItselfTheSheet => Field::Present(words(&[SHOWING, MODAL])),
            _ => showing,
        };
        return Expansion::read(frame, vec![1u32]);
    }
    let (role, children) = match (shape, key) {
        (Window::HoldingADialog, 1) => (DIALOG, Vec::new()),
        (Window::HoldingAPage, 1) => (DOCUMENT_WEB, (2..2 + PAGE_NODES).collect()),
        (Window::HoldingAPage, _) => (DIALOG, Vec::new()),
        _ => (PANEL, Vec::new()),
    };
    let mut child = read_of(role, "", &[]);
    child.node.element = AtspiElement::new(":1.7", format!("/n{key}"), 77);
    child.node.traits.states = showing;
    Expansion::read(child, children)
}

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn full_walk(shape: Window, limits: &WalkLimits) -> Result<Walk<ObservedRead>, AdapterError> {
    walk_levels(
        0u32,
        limits,
        deadline(),
        |level| Ok(level.iter().map(|key| node(*key, shape)).collect()),
        classify,
    )
}

fn sheet_request() -> ObservationRequest {
    let mut sheet = request();
    sheet.surface = SnapshotSurface::Sheet;
    sheet
}

#[derive(Default)]
struct Reads {
    full: Vec<(String, ReadPurpose)>,
    scans: Vec<(String, usize)>,
    scanned: Vec<u32>,
}

fn observe_sheet(shape: Window) -> (Result<(), AdapterError>, Reads) {
    let window = window(window_id::encode(":1.7", "/a").expect("encodes"));
    let mut reads = Reads::default();
    let result = observe_with(
        ObservationRoot::Window(&window),
        &sheet_request(),
        SettleHost {
            walker: &mut |element, limits, _, purpose| {
                reads.full.push((element.path().to_owned(), purpose));
                let shown = if element.path() == "/a" {
                    shape
                } else {
                    Window::Plain
                };
                full_walk(shown, limits)
            },
            scan: &mut |element, limits, _, purpose| {
                assert_eq!(purpose, ReadPurpose::Structure);
                reads
                    .scans
                    .push((element.path().to_owned(), limits.max_raw_depth));
                walk_structure_levels(0u32, limits, deadline(), |level| {
                    reads.scanned.extend_from_slice(level);
                    Ok(level.iter().map(|key| node(*key, shape)).collect())
                })
            },
            place: &mut |_| Ok(()),
            is_web_process: &|_| Ok(false),
            stopped: &|_| false,
            pause: &mut |_| {},
        },
    )
    .map(|_| ());
    (result, reads)
}

fn whole_depth() -> usize {
    WalkLimits::default().max_raw_depth
}

#[test]
fn a_window_without_a_dialog_is_answered_from_structural_reads_alone() {
    let (result, reads) = observe_sheet(Window::Plain);
    let error = result.expect_err("no dialog is shown");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert!(reads.full.is_empty(), "absence needs no full read");
    assert_eq!(
        reads.scans,
        [("/a".to_owned(), 0), ("/a".to_owned(), whole_depth())]
    );
}

#[test]
fn only_the_found_dialog_gets_the_full_read() {
    let (result, reads) = observe_sheet(Window::HoldingADialog);
    result.expect("the nested dialog is the sheet");
    assert_eq!(reads.full, [("/n1".to_owned(), ReadPurpose::Snapshot)]);
}

#[test]
fn a_window_that_is_itself_the_sheet_is_probed_at_its_root_alone() {
    let (result, reads) = observe_sheet(Window::ItselfTheSheet);
    result.expect("the window is the sheet");
    assert_eq!(reads.scans, [("/a".to_owned(), 0)]);
    assert_eq!(reads.scanned, [0], "the probe reads no child of the window");
    assert_eq!(reads.full, [("/a".to_owned(), ReadPurpose::Snapshot)]);
}

#[test]
fn discovery_never_reads_into_a_web_document() {
    let (result, reads) = observe_sheet(Window::HoldingAPage);
    let error = result.expect_err("a dialog inside page content is no sheet");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert!(reads.full.is_empty(), "discovery never takes the full walk");
    assert_eq!(
        reads.scanned,
        [0, 0, 1],
        "the page below the document is never read"
    );
}
