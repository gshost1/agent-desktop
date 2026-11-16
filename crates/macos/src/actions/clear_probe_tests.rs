use super::{ClearProbe, clear_with};
use crate::actions::edit_commit::EditCommit;
use agent_desktop_core::{Action, ActionStep, AdapterError, DeliverySemantics, StepMechanism};
use std::cell::Cell;

fn probe(role: Option<&str>, subrole: Option<&str>, characters: Option<i64>) -> ClearProbe {
    ClearProbe {
        role: role.map(str::to_owned),
        subrole: subrole.map(str::to_owned),
        characters,
    }
}

fn written() -> Result<Vec<ActionStep>, AdapterError> {
    Ok(vec![
        ActionStep::succeeded("AXValue")
            .with_mechanism(StepMechanism::SemanticApi)
            .with_verified(true),
    ])
}

#[test]
fn a_field_that_already_holds_no_text_is_satisfied_without_a_write() {
    let result = clear_with(
        &Action::Clear,
        &probe(Some("AXTextField"), None, Some(0)),
        || panic!("an empty field was written"),
        || panic!("a write that never ran was judged"),
    )
    .unwrap();

    assert_eq!(result.disposition(), DeliverySemantics::not_delivered());
    assert!(
        result
            .details
            .as_ref()
            .is_none_or(|details| details.get("commit_hint").is_none())
    );
}

#[test]
fn a_field_with_text_is_written_and_judged_by_the_roles_already_read() {
    let result = clear_with(
        &Action::Clear,
        &probe(Some("AXTextField"), None, Some(5)),
        written,
        || panic!("the roles were read again after the write"),
    )
    .unwrap();

    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(result.details.unwrap()["application_commit"], "unobserved");
}

#[test]
fn a_secure_field_is_written_even_when_it_reports_no_text() {
    let writes = Cell::new(0);
    clear_with(
        &Action::Clear,
        &probe(Some("AXTextField"), Some("AXSecureTextField"), Some(0)),
        || {
            writes.set(writes.get() + 1);
            written()
        },
        || EditCommit::Immediate,
    )
    .unwrap();

    assert_eq!(writes.get(), 1);
}

#[test]
fn a_count_the_probe_could_not_read_is_no_proof_of_an_empty_field() {
    let writes = Cell::new(0);
    clear_with(
        &Action::Clear,
        &probe(Some("AXComboBox"), None, None),
        || {
            writes.set(writes.get() + 1);
            written()
        },
        || EditCommit::Immediate,
    )
    .unwrap();

    assert_eq!(writes.get(), 1);
}

#[test]
fn an_unread_probe_reads_the_roles_after_the_write() {
    let reads = Cell::new(0);
    let result = clear_with(&Action::Clear, &ClearProbe::default(), written, || {
        reads.set(reads.get() + 1);
        EditCommit::OnEndEditing
    })
    .unwrap();

    assert_eq!(reads.get(), 1);
    assert_eq!(result.details.unwrap()["application_commit"], "unobserved");
}

#[test]
fn a_refused_write_is_the_answer() {
    let error = clear_with(
        &Action::Clear,
        &probe(Some("AXTextField"), None, Some(3)),
        || Err(AdapterError::not_supported("clear")),
        || panic!("a refused write was judged"),
    )
    .unwrap_err();

    assert!(error.is_default_not_supported("clear"));
}
