//! A derived name for a row, tree item or cell that no accessible-name source
//! names but whose only text field holds its label, as file lists and outlines
//! whose rows rename inline report it.
//!
//! The value is a control value, so it never becomes identity: the adapter
//! carries it beside an absent accessible name, and core projects it into the
//! displayed `name` with a `name_source` marker that ref entries skip.

use agent_desktop_core::{LocatorField, NameSource, NodeDescriptor};

use crate::tree::AXElement;
use crate::tree::child_label_read::ChildLabelRead;

const FIELD_NAMED_ROLES: [&str; 3] = ["row", "treeitem", "cell"];

/// The one text field a nameless row, tree item or cell may take its name from.
/// The pass must have read every child it examined and found no static-text
/// label, no secure field and exactly one text field, so a wide element or one
/// with two fields is left unnamed rather than named after a guess.
pub(crate) fn sole_text_field<'a>(role: &str, read: &'a ChildLabelRead) -> Option<&'a AXElement> {
    if !FIELD_NAMED_ROLES.contains(&role)
        || read.label.is_some()
        || !read.complete
        || read.capped
        || read.secure_fields != 0
    {
        return None;
    }
    match read.text_fields.as_slice() {
        [field] => Some(field),
        _ => None,
    }
}

/// The derived name a field yields: none for a secure field, and none unless
/// both reads completed and the value is non-blank text.
pub(crate) fn field_name(
    subrole: Option<&str>,
    subrole_complete: bool,
    value: Option<String>,
    value_complete: bool,
) -> Option<String> {
    if !subrole_complete || !value_complete || subrole == Some("AXSecureTextField") {
        return None;
    }
    value.filter(|text| !text.trim().is_empty())
}

/// The descriptor group carrying a derived name, which is set only where the
/// accessible name was read and found absent.
pub(crate) fn derived_descriptor(
    name: &LocatorField<String>,
    derived: Option<String>,
) -> NodeDescriptor {
    match (name, derived) {
        (LocatorField::Absent, Some(derived)) => NodeDescriptor {
            name_source: Some(NameSource::ChildFieldValue),
            derived_name: Some(derived),
            ..NodeDescriptor::default()
        },
        _ => NodeDescriptor::default(),
    }
}

/// Reads the field's subrole and value. A failed read yields no derived name
/// and leaves the accessible name's certainty alone, because a derived name
/// must never change what identity evidence reports.
pub(crate) fn read_field_name(
    field: &AXElement,
    deadline: std::time::Instant,
    sinks: &mut crate::tree::child_labels::NameEvidenceSinks<'_>,
) -> Result<Option<String>, agent_desktop_core::AdapterError> {
    let (subrole, subrole_complete) =
        crate::tree::child_labels::timed_string(field, "AXSubrole", deadline, sinks)?;
    if subrole.as_deref() == Some("AXSecureTextField") {
        return Ok(None);
    }
    sinks.stats.semantic_reads.child_label_reads += 1;
    let read = crate::tree::properties::attributes::copy_value_typed_bounded_result(
        field,
        deadline,
        sinks.usage,
    );
    let (value, value_complete) = match read {
        Ok(value) => crate::tree::child_labels::complete_text(value, sinks.stats),
        Err(error) => crate::tree::child_labels::degrade_transient_read(
            error,
            "child_field.value",
            sinks.stats,
        )?,
    };
    let Some(name) = field_name(subrole.as_deref(), subrole_complete, value, value_complete) else {
        return Ok(None);
    };
    let (joined, complete) = crate::tree::child_labels::join_unique_labels([name], sinks.usage);
    Ok(joined.filter(|_| complete))
}

#[cfg(test)]
#[path = "child_field_name_tests.rs"]
mod tests;
