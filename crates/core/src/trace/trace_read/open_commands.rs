use super::command_pairing::CommandPairing;
use super::count_bucket::CountBucket;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

type SeqKey<'a> = (&'a str, u64, u64);
type CountKey<'a> = (&'a str, u64, &'a str);

/// The `command.start` events still waiting for their `command.end`, as
/// indexes into the merged event list.
///
/// A start that some end names by `command_seq` waits in `by_seq` and closes
/// only through that end. Every other start waits in a `CountBucket` under
/// its segment, writer pid and command, where a legacy end closes one.
///
/// The claimed set is read in a pass of its own before any pairing. A
/// legacy end can come before the sequenced end that claims a start of the
/// same command (start A, start B, legacy end of B, end of A by seq), so a
/// single pass could not tell, at the legacy end, which starts to leave alone.
#[derive(Debug)]
pub(crate) struct OpenCommands<'a> {
    claimed: HashSet<SeqKey<'a>>,
    by_seq: HashMap<SeqKey<'a>, usize>,
    by_count: HashMap<CountKey<'a>, CountBucket>,
}

impl<'a> OpenCommands<'a> {
    pub(crate) fn new(events: &'a [Value]) -> Self {
        let claimed = events
            .iter()
            .filter(|event| event_name(event) == Some("command.end"))
            .filter_map(|event| match CommandPairing::of_end(event) {
                CommandPairing::BySeq(seq) => Some((segment(event), writer_pid(event), seq)),
                CommandPairing::LegacyCount => None,
            })
            .collect();
        Self {
            claimed,
            by_seq: HashMap::new(),
            by_count: HashMap::new(),
        }
    }

    pub(crate) fn start(&mut self, index: usize, event: &'a Value) {
        let seq_key = event
            .get("seq")
            .and_then(Value::as_u64)
            .map(|seq| (segment(event), writer_pid(event), seq))
            .filter(|key| self.claimed.contains(key));
        match seq_key {
            Some(key) => {
                self.by_seq.insert(key, index);
            }
            None => self
                .by_count
                .entry((segment(event), writer_pid(event), command(event)))
                .or_default()
                .push(index),
        }
    }

    pub(crate) fn end(&mut self, event: &'a Value) -> bool {
        match CommandPairing::of_end(event) {
            CommandPairing::BySeq(seq) => self
                .by_seq
                .remove(&(segment(event), writer_pid(event), seq))
                .is_some(),
            CommandPairing::LegacyCount => self
                .by_count
                .get_mut(&(segment(event), writer_pid(event), command(event)))
                .is_some_and(CountBucket::close),
        }
    }

    /// The starts left open, in event order, each with whether its identity
    /// is known. It is unknown only for starts left in a bucket whose legacy
    /// ends had to guess.
    pub(crate) fn into_unpaired(self) -> Vec<(usize, bool)> {
        let mut unpaired: Vec<(usize, bool)> = self
            .by_seq
            .into_values()
            .map(|index| (index, true))
            .chain(
                self.by_count
                    .into_values()
                    .flat_map(CountBucket::into_unpaired),
            )
            .collect();
        unpaired.sort_unstable();
        unpaired
    }
}

pub(crate) fn event_name(event: &Value) -> Option<&str> {
    event.get("event").and_then(Value::as_str)
}

pub(crate) fn segment(event: &Value) -> &str {
    event.get("segment").and_then(Value::as_str).unwrap_or("")
}

pub(crate) fn writer_pid(event: &Value) -> u64 {
    event.get("writer_pid").and_then(Value::as_u64).unwrap_or(0)
}

pub(crate) fn command(event: &Value) -> &str {
    event
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}
