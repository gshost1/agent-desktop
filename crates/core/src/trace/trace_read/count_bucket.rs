use std::collections::VecDeque;

/// The open `command.start` events of one segment, writer pid and command
/// that only legacy ends (no `command_seq`) can close.
///
/// A legacy end closes the oldest open start. When it closes one of
/// several, the choice is a guess and `ambiguous` latches: the starts
/// left open are then known only by count, never by seq. While every close
/// found exactly one open start, each pairing was forced and the starts
/// left open are exactly the unpaired ones.
#[derive(Debug, Default)]
pub(crate) struct CountBucket {
    open: VecDeque<usize>,
    ambiguous: bool,
}

impl CountBucket {
    pub(crate) fn push(&mut self, index: usize) {
        self.open.push_back(index);
    }

    pub(crate) fn close(&mut self) -> bool {
        if self.open.len() > 1 {
            self.ambiguous = true;
        }
        self.open.pop_front().is_some()
    }

    pub(crate) fn into_unpaired(self) -> impl Iterator<Item = (usize, bool)> {
        let known = !self.ambiguous;
        self.open.into_iter().map(move |index| (index, known))
    }
}
