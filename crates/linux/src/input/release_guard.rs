use agent_desktop_core::AdapterError;

use super::emergency_release::EmergencyRelease;
use super::held_input::HeldInput;

use super::keyboard::keymap_edit::KeymapEdit;
use super::release_state::ReleaseState;
use super::x11::held_journal::HeldJournal;
use super::x11::sink::X11Sink;
use super::x11::sink_error::SinkError;

pub(crate) struct ReleaseGuard<'a> {
    sink: &'a mut dyn X11Sink,
    state: ReleaseState,
    held: HeldInput,
    touched: HeldInput,
    origin: Option<(i32, i32)>,
    remaps: Vec<KeymapEdit>,
    journal: Option<HeldJournal>,
}

fn remember(list: &mut Vec<u8>, code: u8) {
    if !list.contains(&code) {
        list.push(code);
    }
}

fn pointer_point(x: i32, y: i32) -> Result<(i16, i16), SinkError> {
    let coordinate = |value: i32| {
        i16::try_from(value)
            .map_err(|_| SinkError::Request("pointer coordinate out of range".into()))
    };
    Ok((coordinate(x)?, coordinate(y)?))
}

impl<'a> ReleaseGuard<'a> {
    pub(crate) fn new(sink: &'a mut dyn X11Sink) -> Self {
        Self {
            sink,
            state: ReleaseState::default(),
            held: HeldInput::default(),
            touched: HeldInput::default(),
            origin: None,
            remaps: Vec::new(),
            journal: None,
        }
    }

    pub(crate) fn journal_to(&mut self, journal: Option<HeldJournal>) {
        self.journal = journal;
    }

    fn sync_journal(&self) -> Result<(), SinkError> {
        match &self.journal {
            Some(journal) => journal
                .record(&self.held, &self.remaps)
                .map_err(|error| SinkError::Journal(error.to_string())),
            None => Ok(()),
        }
    }

    pub(crate) fn set_origin(&mut self, origin: Option<(i32, i32)>) {
        self.origin = origin;
    }

    pub(crate) fn delivered(&self) -> u32 {
        self.state.delivered()
    }

    pub(crate) fn touched(&self) -> &HeldInput {
        &self.touched
    }

    pub(crate) fn sink(&mut self) -> &mut dyn X11Sink {
        self.sink
    }

    pub(crate) fn key(&mut self, code: u8, down: bool) -> Result<(), SinkError> {
        self.edge(true, code, down)
    }

    pub(crate) fn button(&mut self, code: u8, down: bool) -> Result<(), SinkError> {
        self.edge(false, code, down)
    }

    fn edge(&mut self, keyboard: bool, code: u8, down: bool) -> Result<(), SinkError> {
        if down {
            let (held, touched) = if keyboard {
                (&mut self.held.keys, &mut self.touched.keys)
            } else {
                (&mut self.held.buttons, &mut self.touched.buttons)
            };
            let was_held = held.contains(&code);
            remember(held, code);
            remember(touched, code);
            if let Err(error) = self.sync_journal() {
                if !was_held {
                    self.remove_held(keyboard, code);
                }
                return Err(error);
            }
        }
        self.state.mark_started();
        if keyboard {
            self.sink.key(code, down)?;
        } else {
            self.sink.button(code, down)?;
        }
        self.state.mark_delivered();
        if !down {
            self.sink.round_trip()?;
            self.remove_held(keyboard, code);
            let _ = self.sync_journal();
        }
        Ok(())
    }

    fn remove_held(&mut self, keyboard: bool, code: u8) {
        let held = if keyboard {
            &mut self.held.keys
        } else {
            &mut self.held.buttons
        };
        held.retain(|held| *held != code);
    }

    pub(crate) fn move_to(&mut self, x: i32, y: i32) -> Result<(), SinkError> {
        let point = pointer_point(x, y)?;
        self.state.mark_started();
        self.send_move(point)?;
        self.state.mark_delivered();
        Ok(())
    }

    fn send_move(&mut self, (x, y): (i16, i16)) -> Result<(), SinkError> {
        self.sink.move_to(x, y)
    }

    pub(crate) fn edit_keymap(&mut self, edit: &KeymapEdit) -> Result<(), SinkError> {
        let current = self.sink.keymap()?;
        let original = current
            .keysyms_of(edit.keycode)
            .ok_or(SinkError::Unrestorable(edit.keycode))?;
        let restoring = self
            .remaps
            .iter()
            .position(|saved| saved.keycode == edit.keycode && saved.keysyms == edit.keysyms);
        self.track_remap(edit, original);
        self.sync_journal()?;
        self.state.mark_started();
        self.sink.edit_keymap(edit)?;
        self.sink.round_trip()?;
        if let Some(index) = restoring {
            self.remaps.remove(index);
            let _ = self.sync_journal();
        }
        self.state.mark_delivered();
        Ok(())
    }

    fn track_remap(&mut self, edit: &KeymapEdit, original: Vec<u32>) {
        if original == edit.keysyms
            || self
                .remaps
                .iter()
                .any(|saved| saved.keycode == edit.keycode)
        {
            return;
        }
        self.remaps.push(KeymapEdit {
            keycode: edit.keycode,
            keysyms: original,
        });
    }

    /// Ends the sequence after `error`: posts corrective input only for what is still held or
    /// remapped, then reports the error with the release's real outcome.
    pub(crate) fn abort(mut self, error: AdapterError, suggestion: &str) -> AdapterError {
        let release = if self.holding() {
            let acknowledged = self.release_everything().is_ok();
            self.forget();
            EmergencyRelease::Posted { acknowledged }
        } else {
            let _ = self.sync_journal();
            EmergencyRelease::NotNeeded
        };
        self.state.enrich_error(error, release, suggestion)
    }

    fn holding(&self) -> bool {
        !self.held.keys.is_empty() || !self.held.buttons.is_empty() || !self.remaps.is_empty()
    }

    /// Drops the in-memory record of held input and remaps without touching the journal. After a
    /// failed emergency release, the journal's stale sweep owns what is left, so dropping the guard
    /// does not post the same release a second time.
    fn forget(&mut self) {
        self.held = HeldInput::default();
        self.remaps.clear();
    }

    /// Releases every held button and key, restores every remap, and confirms with a round trip.
    /// The guard keeps its record, and the journal, until the server acknowledges the release.
    pub(crate) fn release_everything(&mut self) -> Result<(), SinkError> {
        let mut outcome = Ok(());
        if !self.held.buttons.is_empty() {
            if let Some((x, y)) = self.origin {
                let back = pointer_point(x, y).and_then(|point| self.send_move(point));
                keep_first(&mut outcome, back);
            }
            for code in self.held.buttons.iter().rev() {
                keep_first(&mut outcome, self.sink.button(*code, false));
            }
        }
        for code in self.held.keys.iter().rev() {
            keep_first(&mut outcome, self.sink.key(*code, false));
        }
        for edit in self.remaps.iter().rev() {
            keep_first(&mut outcome, self.sink.edit_keymap(edit));
        }
        keep_first(&mut outcome, self.sink.round_trip());
        if outcome.is_ok() {
            self.forget();
            let _ = self.sync_journal();
        }
        outcome
    }
}

fn keep_first(outcome: &mut Result<(), SinkError>, next: Result<(), SinkError>) {
    if outcome.is_ok() {
        *outcome = next;
    }
}

impl Drop for ReleaseGuard<'_> {
    fn drop(&mut self) {
        if self.holding() {
            let _ = self.release_everything();
        }
    }
}

#[cfg(test)]
#[path = "release_guard_tests.rs"]
mod tests;
