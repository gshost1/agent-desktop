//! `select` on a table or outline row replaces the container's selection when
//! it exposes a writable `AXSelectedRows`, or selects the row directly.
//! The value must match one of the row's labels before any write.

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

use crate::actions::chain::chain_delivery::DeliveryOutcome;

const MAX_LABEL_NODES: usize = 48;
const MAX_LABEL_DEPTH: usize = 3;
const MAX_LABEL_CHILDREN: usize = 16;
const LABEL_ATTRIBUTES: [&str; 3] = ["AXValue", "AXTitle", "AXDescription"];

fn labels_match(labels: &[String], value: &str) -> bool {
    let wanted = value.trim();
    !wanted.is_empty()
        && labels
            .iter()
            .any(|label| label.trim().eq_ignore_ascii_case(wanted))
}

fn verified_from_outcome(outcome: DeliveryOutcome) -> Result<DeliveryOutcome, AdapterError> {
    match outcome {
        DeliveryOutcome::DeliveredVerified | DeliveryOutcome::SatisfiedNoDelivery => Ok(outcome),
        DeliveryOutcome::NotDelivered => Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Neither the row nor its table or outline accepts a selection write",
        )
        .with_disposition(DeliverySemantics::not_delivered())
        .with_suggestion("Click the row instead.")),
        DeliveryOutcome::DeliveredUnverified | DeliveryOutcome::DeliveredWithoutEffect => {
            Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The selection write was delivered but the row did not read back as selected",
            )
            .with_disposition(DeliverySemantics::delivered_unverified())
            .with_suggestion("Inspect the selection before deciding whether to act again."))
        }
    }
}

fn select_row_with(
    selected: Option<bool>,
    write: impl FnOnce() -> Result<DeliveryOutcome, AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    if selected == Some(true) {
        return Ok(DeliveryOutcome::SatisfiedNoDelivery);
    }
    verified_from_outcome(write()?)
}

fn no_matching_label(value: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ElementNotFound,
        format!("The targeted row has no label matching '{value}'"),
    )
    .with_disposition(DeliverySemantics::not_delivered())
    .with_suggestion("Pass the row's name or one of its cell texts as the snapshot shows them.")
}

mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::tree::AXElement;

    use super::{
        DeliveryOutcome, LABEL_ATTRIBUTES, MAX_LABEL_CHILDREN, MAX_LABEL_DEPTH, MAX_LABEL_NODES,
    };
    use crate::actions::select::container_select::write_row_selection;
    use crate::actions::select::row_selection::member_selected;

    pub(crate) fn select_row(
        row: &AXElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        if !super::labels_match(&row_labels(row, deadline), value) {
            return Err(super::no_matching_label(value));
        }
        super::select_row_with(member_selected(row, deadline), || {
            write_row_selection(row, deadline)
        })
    }

    fn row_labels(row: &AXElement, deadline: Deadline) -> Vec<String> {
        let mut labels = Vec::new();
        let mut frontier = vec![row.clone()];
        let mut visited = 0;
        for _ in 0..=MAX_LABEL_DEPTH {
            let mut next = Vec::new();
            for node in &frontier {
                if visited >= MAX_LABEL_NODES || deadline.is_expired() {
                    return labels;
                }
                visited += 1;
                labels.extend(LABEL_ATTRIBUTES.iter().filter_map(|attribute| {
                    crate::tree::properties::attributes::copy_string_attr_result(
                        node, attribute, deadline,
                    )
                    .ok()
                    .flatten()
                }));
                next.extend(
                    crate::tree::properties::attributes::copy_ax_array_prefix_result(
                        node,
                        "AXChildren",
                        MAX_LABEL_CHILDREN,
                        deadline,
                    )
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
                );
            }
            frontier = next;
        }
        labels
    }
}

pub(crate) use imp::select_row;

#[cfg(test)]
#[path = "select_row_tests.rs"]
mod tests;
