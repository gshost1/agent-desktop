use crate::locator::LocatorQuery;

use super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, RefEvidenceRequirements,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvidenceRequirements {
    pub role: bool,
    pub name: bool,
    pub description: bool,
    pub value: bool,
    pub identifiers: bool,
    pub states: bool,
    pub ref_evidence: RefEvidenceRequirements,
}

impl EvidenceRequirements {
    pub fn snapshot() -> Self {
        Self {
            role: true,
            name: true,
            description: true,
            value: true,
            identifiers: true,
            states: true,
            ref_evidence: RefEvidenceRequirements {
                bounds: true,
                actions: true,
            },
        }
    }

    pub(crate) fn locator(query: &LocatorQuery, request: &LocatorResolveRequest) -> Self {
        if request.materialization == LocatorMaterialization::FullRefMap {
            return Self::snapshot();
        }
        if request.materialization == LocatorMaterialization::SelectedMatches
            && query.reads_subtrees()
        {
            return Self::snapshot();
        }
        let mut requirements = Self::query(query);
        if request.materialization == LocatorMaterialization::SelectedMatches {
            requirements.identifiers = true;
            requirements.ref_evidence.bounds = true;
        }
        requirements
    }

    /// The evidence a locator walk reads below its root. A `--first`,
    /// `--last` or `--nth` text query publishes one match, so only the root
    /// keeps snapshot evidence: a root match publishes from the window read,
    /// and any deeper match is hydrated over its own subtree, the re-read
    /// that checks its witness. Every other node is read for the query, its
    /// identity and its bounds, as a role-and-name walk reads it, instead of
    /// for the actions and states only a published match uses. A text query
    /// reads every node's name and description, so the root's capture proof
    /// still counts its twins; a containment query without text may not, so
    /// it keeps snapshot evidence everywhere.
    pub(crate) fn locator_descendants(
        query: &LocatorQuery,
        request: &LocatorResolveRequest,
    ) -> Self {
        let publishes_one = matches!(
            request.selection,
            LocatorSelection::First | LocatorSelection::Last | LocatorSelection::Nth(_)
        );
        if publishes_one
            && request.materialization == LocatorMaterialization::SelectedMatches
            && query.has_text.is_some()
        {
            let mut requirements = Self::query(query);
            requirements.identifiers = true;
            requirements.ref_evidence.bounds = true;
            return requirements;
        }
        Self::locator(query, request)
    }

    pub(crate) fn query(query: &LocatorQuery) -> Self {
        let needs_subtree_text = query.has_text.is_some();
        let mut requirements = Self {
            role: true,
            name: query.identity.name.is_some() || needs_subtree_text,
            description: query.identity.description.is_some() || needs_subtree_text,
            value: query.identity.value.is_some() || needs_subtree_text,
            identifiers: query.identity.native_id.is_some(),
            states: !query.states.is_empty(),
            ref_evidence: RefEvidenceRequirements::default(),
        };
        for nested in [
            query.containment.has.as_deref(),
            query.containment.has_not.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            requirements = requirements.union(Self::query(nested));
        }
        requirements
    }

    fn union(self, other: Self) -> Self {
        Self {
            role: self.role || other.role,
            name: self.name || other.name,
            description: self.description || other.description,
            value: self.value || other.value,
            identifiers: self.identifiers || other.identifiers,
            states: self.states || other.states,
            ref_evidence: self.ref_evidence.union(other.ref_evidence),
        }
    }

    pub(super) fn covers(self, other: Self) -> bool {
        self.union(other) == self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locator::live_locator::LocatorSelection;

    #[test]
    fn selected_matches_use_typed_identifiers_and_bounds_without_global_semantic_reads() {
        let query = LocatorQuery {
            identity: crate::IdentityPredicate {
                role: Some("button".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let request = LocatorResolveRequest {
            selection: LocatorSelection::First,
            deadline: crate::Deadline::after(500).unwrap(),
            max_raw_depth: 50,
            surface: None,
            materialization: LocatorMaterialization::SelectedMatches,
        };

        let requirements = EvidenceRequirements::locator(&query, &request);

        assert!(requirements.role);
        assert!(!requirements.name);
        assert!(!requirements.description);
        assert!(!requirements.value);
        assert!(requirements.identifiers);
        assert!(requirements.ref_evidence.bounds);
        assert!(!requirements.states);
        assert!(!requirements.ref_evidence.actions);
    }

    #[test]
    fn selected_subtree_matches_read_snapshot_evidence_so_they_publish_from_one_read() {
        let query = LocatorQuery {
            has_text: Some("needle".into()),
            ..Default::default()
        };
        let request = LocatorResolveRequest {
            selection: LocatorSelection::All { limit: Some(10) },
            deadline: crate::Deadline::after(500).unwrap(),
            max_raw_depth: 50,
            surface: None,
            materialization: LocatorMaterialization::SelectedMatches,
        };
        assert_eq!(
            EvidenceRequirements::locator(&query, &request),
            EvidenceRequirements::snapshot()
        );
        let unpublished = LocatorResolveRequest {
            materialization: LocatorMaterialization::None,
            ..request
        };
        assert!(!EvidenceRequirements::locator(&query, &unpublished).states);
        assert_eq!(
            EvidenceRequirements::locator_descendants(&query, &request),
            EvidenceRequirements::snapshot(),
            "a query that publishes many matches still reads them whole"
        );
    }

    /// A text query that publishes one match reads actions and states only at
    /// the root. Reading them on every node made `find --text --first`
    /// several times a role-and-name find on a platform that honors the plan.
    #[test]
    fn a_single_subtree_match_reads_snapshot_evidence_only_at_the_root() {
        let query = LocatorQuery {
            has_text: Some("needle".into()),
            ..Default::default()
        };
        for selection in [
            LocatorSelection::First,
            LocatorSelection::Last,
            LocatorSelection::Nth(2),
        ] {
            let request = LocatorResolveRequest {
                selection,
                deadline: crate::Deadline::after(500).unwrap(),
                max_raw_depth: 50,
                surface: None,
                materialization: LocatorMaterialization::SelectedMatches,
            };
            assert_eq!(
                EvidenceRequirements::locator(&query, &request),
                EvidenceRequirements::snapshot()
            );
            let descendants = EvidenceRequirements::locator_descendants(&query, &request);
            assert!(descendants.name && descendants.description && descendants.value);
            assert!(descendants.identifiers && descendants.ref_evidence.bounds);
            assert!(
                !descendants.states && !descendants.ref_evidence.actions,
                "{selection:?} publishes one match and must not read every node's actions"
            );
        }
    }

    /// A containment query without text need not read names or descriptions,
    /// which the root's capture proof counts, so it keeps snapshot evidence
    /// on every node.
    #[test]
    fn a_containment_query_without_text_keeps_snapshot_evidence_below_the_root() {
        let query = LocatorQuery {
            containment: crate::ContainmentPredicate {
                has: Some(Box::new(LocatorQuery {
                    identity: crate::IdentityPredicate {
                        role: Some("button".into()),
                        ..Default::default()
                    },
                    ..Default::default()
                })),
                has_not: None,
            },
            ..Default::default()
        };
        let request = LocatorResolveRequest {
            selection: LocatorSelection::First,
            deadline: crate::Deadline::after(500).unwrap(),
            max_raw_depth: 50,
            surface: None,
            materialization: LocatorMaterialization::SelectedMatches,
        };
        assert_eq!(
            EvidenceRequirements::locator_descendants(&query, &request),
            EvidenceRequirements::snapshot()
        );
    }
}
