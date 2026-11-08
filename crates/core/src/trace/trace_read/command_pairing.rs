use serde_json::Value;

/// How a `command.end` names the `command.start` it closes.
///
/// `BySeq` holds the start's `seq`, read from the end's `command_seq` field.
/// Two scopes of one command can overlap in one process (FFI host threads),
/// so only the start's own `seq` identifies which scope ended.
/// `LegacyCount` is an end written before `command_seq` existed. It closes
/// one open start with the same segment, writer pid and command, so
/// overlapping legacy scopes pair by count rather than by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandPairing {
    BySeq(u64),
    LegacyCount,
}

impl CommandPairing {
    pub(crate) fn of_end(event: &Value) -> Self {
        match event.get("command_seq").and_then(Value::as_u64) {
            Some(seq) => Self::BySeq(seq),
            None => Self::LegacyCount,
        }
    }
}
