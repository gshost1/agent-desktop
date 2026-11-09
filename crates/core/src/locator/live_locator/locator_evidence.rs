use super::{EvidenceRequirements, IdentifierEvidence, LocatorField, LocatorRefEvidence};

#[derive(Debug, Clone, PartialEq)]
pub struct LocatorEvidence {
    pub role: LocatorField<String>,
    pub name: LocatorField<String>,
    pub description: LocatorField<String>,
    pub value: LocatorField<String>,
    pub identifiers: IdentifierEvidence,
    pub states: LocatorField<Vec<String>>,
    pub ref_evidence: LocatorRefEvidence,
}

impl LocatorEvidence {
    pub fn satisfies(&self, requirements: EvidenceRequirements) -> bool {
        (!requirements.role || !self.role.is_unknown())
            && (!requirements.name || !self.name.is_unknown())
            && (!requirements.description || !self.description.is_unknown())
            && (!requirements.value || !self.value.is_unknown())
            && (!requirements.identifiers || self.identifiers.is_complete())
            && (!requirements.states || !self.states.is_unknown())
            && (!requirements.ref_evidence.bounds || !self.ref_evidence.bounds.is_unknown())
            && (!requirements.ref_evidence.actions
                || !self.ref_evidence.available_actions.is_unknown())
    }

    /// The derived name an adapter read for an element no accessible-name
    /// source named. It applies only while the accessible name was read and
    /// found absent, and it locates and displays the element only: `name` stays
    /// absent, so ref identity and capture proofs never read it.
    pub(crate) fn derived_name(&self) -> Option<&str> {
        match self.name {
            LocatorField::Absent => crate::tree::accname::non_blank(
                self.ref_evidence.descriptors.derived_name.as_deref(),
            ),
            LocatorField::Known(_) | LocatorField::Unknown => None,
        }
    }

    /// The name a match response or projected node displays.
    pub(crate) fn display_name(&self) -> Option<String> {
        self.name
            .meaningful_string()
            .or_else(|| self.derived_name().map(str::to_owned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_requirements_reject_unknown_identity_and_ref_evidence() {
        let mut evidence =
            crate::locator::live_locator::test_support::evidence("button", Some("Save"));
        assert!(evidence.satisfies(EvidenceRequirements::snapshot()));

        evidence.name = LocatorField::Unknown;
        assert!(!evidence.satisfies(EvidenceRequirements::snapshot()));
        evidence.name = LocatorField::Known("Save".into());
        evidence.ref_evidence.bounds = LocatorField::Unknown;
        assert!(!evidence.satisfies(EvidenceRequirements::snapshot()));
    }
}
