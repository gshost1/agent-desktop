use super::command_pairing::CommandPairing;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

type SeqKey<'a> = (&'a str, u64, u64);
type CountKey<'a> = (&'a str, u64, &'a str);

/// The `command.start` events still waiting for their `command.end`, as
/// indexes into the merged event list.
///
/// A start that some end names by `command_seq` waits in `by_seq` and closes
/// only through that end. Every other start waits in `by_count` under its
/// segment, writer pid and command, where a legacy end closes one of them.
/// Holding both apart keeps a legacy end from closing a start that a
/// sequenced end will claim later.
#[derive(Debug)]
pub(crate) struct OpenCommands<'a> {
    claimed: HashSet<SeqKey<'a>>,
    by_seq: HashMap<SeqKey<'a>, usize>,
    by_count: HashMap<CountKey<'a>, Vec<usize>>,
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
        match self.pairing_of_start(event) {
            CommandPairing::BySeq(seq) => {
                self.by_seq
                    .insert((segment(event), writer_pid(event), seq), index);
            }
            CommandPairing::LegacyCount => self
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
                .and_then(Vec::pop)
                .is_some(),
        }
    }

    pub(crate) fn into_unpaired(self) -> Vec<usize> {
        let mut unpaired: Vec<usize> = self
            .by_seq
            .into_values()
            .chain(self.by_count.into_values().flatten())
            .collect();
        unpaired.sort_unstable();
        unpaired
    }

    fn pairing_of_start(&self, event: &'a Value) -> CommandPairing {
        match event.get("seq").and_then(Value::as_u64) {
            Some(seq)
                if !self
                    .by_seq
                    .contains_key(&(segment(event), writer_pid(event), seq))
                    && self
                        .claimed
                        .contains(&(segment(event), writer_pid(event), seq)) =>
            {
                CommandPairing::BySeq(seq)
            }
            _ => CommandPairing::LegacyCount,
        }
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
