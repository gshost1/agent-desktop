use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{
    CaptureVerdict, ElementIdentifier, IdentityMatch, RefEntry,
    locator::live_locator::{LocatorEvidence, LocatorField, ObservationSource, ObservedTree},
    refs::ref_full_predicate::{DESCRIPTION, FullPredicate, NAME, NATIVE_ID},
    refs::ref_identity::stable_name,
    refs::ref_live_predicate::LivePredicate,
    tree::accname::non_blank,
};

const ROLE: u8 = DESCRIPTION << 1;
const MASKS: usize = (NATIVE_ID | NAME | DESCRIPTION) as usize + 1;

/// One whole-window observation, counted once into the field subsets its
/// entries record, so every tokenless entry published from it is judged by the
/// rule `verdict` applies rather than by a fresh scan per entry.
///
/// The count is the comparison resolution runs: `FullPredicate::of_entry` of the
/// entry being published, measured against the evidence every node reported, so
/// an identifier chosen by a query or a field that went unread counts the way it
/// will count later.
pub(crate) struct CaptureProof<'a> {
    observed: &'a ObservedTree,
    start: Option<usize>,
    buckets: [OnceLock<HashMap<LiveKey<'a>, Option<usize>>>; MASKS],
    unread: Vec<(u8, LivePredicate<'a>)>,
    whole_window: bool,
}

impl<'a> CaptureProof<'a> {
    pub(crate) fn new(observed: &'a ObservedTree) -> Self {
        let whole_window = matches!(
            observed.source,
            ObservationSource::Window { .. } | ObservationSource::Application { .. }
        ) && observed.is_exhaustive();
        Self {
            observed,
            whole_window,
            start: observed.roots.first().map(|root| *root as usize),
            buckets: std::array::from_fn(|_| OnceLock::new()),
            unread: observed
                .nodes
                .iter()
                .filter_map(|node| {
                    let bits = unread_bits(&node.evidence);
                    (bits != 0).then(|| (bits, LivePredicate::observed(&node.evidence)))
                })
                .collect(),
        }
    }

    /// Starts allocation paths at the node whose observed path is `prefix`, as a
    /// drill does: the prefix is that node's own path, and the rest of an
    /// allocation path counts positions in the projected tree below it.
    pub(crate) fn rooted_at(mut self, prefix: &[usize]) -> Self {
        self.start = self
            .observed
            .nodes
            .iter()
            .position(|node| node.path.as_slice() == prefix);
        self
    }

    /// The verdict on `entry`, allocated at `path`. The path is read the way
    /// allocation wrote it: the start node's own path, then child positions in
    /// the projection, which skips no child the observation kept even when the
    /// native indices have gaps. A path the observation does not hold cannot tie
    /// the entry to a node, so it is judged by what the observation could say.
    pub(crate) fn verdict_at(&self, entry: &RefEntry, path: &[usize]) -> CaptureVerdict {
        let Some(mut index) = self.start else {
            return self.untied();
        };
        let Some(prefix) = self
            .observed
            .nodes
            .get(index)
            .map(|node| node.path.as_slice())
        else {
            return self.untied();
        };
        let Some(steps) = path.strip_prefix(prefix) else {
            return self.untied();
        };
        for step in steps {
            match self
                .observed
                .nodes
                .get(index)
                .and_then(|node| node.children.get(*step))
            {
                Some(child) => index = *child as usize,
                None => return self.untied(),
            }
        }
        self.verdict(entry, index)
    }

    /// The verdict on `entry` at the node it was built from. Matches are
    /// counted first: another element matching the entry's full predicate is a
    /// twin, `Ambiguous`, even in a partial read, because more reading cannot
    /// remove it. Without one, a read that is not a complete whole-window read
    /// is `Incomplete`. It is `Unique` only when this node is the one match and
    /// no node leaves a recorded field unread, and `Unknown` otherwise.
    pub(crate) fn verdict(&self, entry: &RefEntry, index: usize) -> CaptureVerdict {
        let predicate = FullPredicate::of_entry(entry);
        let fields = predicate.mask();
        let matched = self.buckets[fields as usize]
            .get_or_init(|| count_nodes(self.observed, fields))
            .get(&key_of(&predicate))
            .copied();
        match matched {
            Some(None) => CaptureVerdict::Ambiguous,
            Some(Some(found)) if found != index => CaptureVerdict::Ambiguous,
            _ if !self.whole_window => CaptureVerdict::Incomplete,
            Some(Some(_)) if !self.leaves_unread(&predicate, fields) => CaptureVerdict::Unique,
            _ => CaptureVerdict::Unknown,
        }
    }

    fn leaves_unread(&self, predicate: &FullPredicate<'_>, fields: u8) -> bool {
        self.unread.iter().any(|(bits, live)| {
            bits & (fields | ROLE) != 0 && predicate.compare(live) == IdentityMatch::Unknown
        })
    }

    /// The verdict on an entry the observation holds no node for.
    fn untied(&self) -> CaptureVerdict {
        if self.whole_window {
            CaptureVerdict::Unknown
        } else {
            CaptureVerdict::Incomplete
        }
    }
}

/// The node each full predicate of this field subset matched: its index when
/// exactly one node matched, `None` when two or more did.
fn count_nodes(observed: &ObservedTree, fields: u8) -> HashMap<LiveKey<'_>, Option<usize>> {
    let mut counted: HashMap<LiveKey<'_>, (usize, usize)> = HashMap::new();
    for (index, node) in observed.nodes.iter().enumerate() {
        for key in keys_of(&node.evidence, fields) {
            let entry = counted.entry(key).or_insert((0, index));
            entry.0 += 1;
            entry.1 = index;
        }
    }
    counted
        .into_iter()
        .map(|(key, (count, index))| (key, (count == 1).then_some(index)))
        .collect()
}

fn keys_of<'a>(evidence: &'a LocatorEvidence, fields: u8) -> Vec<LiveKey<'a>> {
    let Some(role) = non_blank(evidence.role.known().map(String::as_str)) else {
        return Vec::new();
    };
    let name = match fields & NAME {
        0 => None,
        _ => match stable_name(
            role,
            evidence.name.known().map(String::as_str),
            evidence.value.known().map(String::as_str),
        ) {
            Some(name) => Some(name),
            None => return Vec::new(),
        },
    };
    let description = match fields & DESCRIPTION {
        0 => None,
        _ => match non_blank(evidence.description.known().map(String::as_str)) {
            Some(description) => Some(description),
            None => return Vec::new(),
        },
    };
    let key = |native_id: Option<&'a ElementIdentifier>| LiveKey {
        role,
        name,
        description,
        native_id,
    };
    if fields & NATIVE_ID == 0 {
        return vec![key(None)];
    }
    evidence
        .identifiers
        .identifiers()
        .iter()
        .map(|native_id| key(Some(native_id)))
        .collect()
}

/// The fields this element could not answer, so a predicate that records one of
/// them reaches `Unknown` here exactly as it will at resolution.
fn unread_bits(evidence: &LocatorEvidence) -> u8 {
    let live = LivePredicate::observed(evidence);
    let mut bits = 0;
    match &live.role {
        LocatorField::Known(role) => {
            if live.stable_name(role).is_unknown() {
                bits |= NAME;
            }
        }
        LocatorField::Unknown => bits |= ROLE,
        LocatorField::Absent => {}
    }
    if live.description.is_unknown() {
        bits |= DESCRIPTION;
    }
    if !live.native_ids_complete {
        bits |= NATIVE_ID;
    }
    bits
}

fn key_of<'b>(predicate: &FullPredicate<'b>) -> LiveKey<'b> {
    LiveKey {
        role: predicate.role,
        name: predicate.name,
        description: predicate.description,
        native_id: predicate.native_id,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct LiveKey<'a> {
    role: &'a str,
    name: Option<&'a str>,
    description: Option<&'a str>,
    native_id: Option<&'a ElementIdentifier>,
}

#[cfg(test)]
#[path = "ref_capture_proof_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ref_capture_proof_verdict_tests.rs"]
mod verdict_tests;
