use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ObservationRequest, ObservationRoot, SnapshotSurface,
};

use super::super::tests::{request, window};
use super::super::{SettleHost, observe_with};
use crate::system::window::window_id;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::walk::walker::walk_levels;
use crate::tree::walk::wrapper::classify;
use crate::tree::{AtspiElement, Expansion, Field, ObservedRead, Walk, WalkLimits};

const SHOWING: u32 = 25;
const DIALOG: u32 = 16;
const FRAME: u32 = 23;

fn frame_walk(dialog: bool, limits: &WalkLimits) -> Result<Walk<ObservedRead>, AdapterError> {
    walk_levels(
        0u32,
        limits,
        Deadline::after(5_000).expect("deadline"),
        |level| {
            Ok(level
                .iter()
                .map(|key| {
                    if *key == 0 {
                        let mut frame = read_of(FRAME, "w", &[]);
                        frame.node.traits.states = Field::Present(words(&[SHOWING]));
                        Expansion::read(frame, vec![1u32])
                    } else {
                        let role = if dialog { DIALOG } else { 39 };
                        let mut child = read_of(role, "", &[]);
                        child.node.element = AtspiElement::new(":1.7", "/d", 77);
                        child.node.traits.states = Field::Present(words(&[SHOWING]));
                        Expansion::read(child, Vec::new())
                    }
                })
                .collect())
        },
        classify,
    )
}

fn sheet_request() -> ObservationRequest {
    let mut sheet = request();
    sheet.surface = SnapshotSurface::Sheet;
    sheet
}

fn observe_sheet(dialog: bool) -> (Result<(), AdapterError>, Vec<(String, ReadPurpose)>) {
    let window = window(window_id::encode(":1.7", "/a").expect("encodes"));
    let mut reads = Vec::new();
    let result = observe_with(
        ObservationRoot::Window(&window),
        &sheet_request(),
        SettleHost {
            walker: &mut |element, limits, _, purpose| {
                reads.push((element.path().to_owned(), purpose));
                frame_walk(dialog && element.path() == "/a", limits)
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

#[test]
fn a_window_without_a_dialog_is_answered_from_one_structural_read() {
    let (result, reads) = observe_sheet(false);
    let error = result.expect_err("no dialog is shown");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(reads, [("/a".to_owned(), ReadPurpose::Structure)]);
}

#[test]
fn only_the_found_dialog_gets_the_full_read() {
    let (result, reads) = observe_sheet(true);
    result.expect("the nested dialog is the sheet");
    assert_eq!(
        reads,
        [
            ("/a".to_owned(), ReadPurpose::Structure),
            ("/d".to_owned(), ReadPurpose::Snapshot)
        ]
    );
}
