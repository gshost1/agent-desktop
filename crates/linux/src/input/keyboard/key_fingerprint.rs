use super::keymap::Keymap;

/// The keysyms each keycode a text plan presses carried when the text was
/// planned. A layout replaced between planning and delivery can leave the
/// group and the modifiers as they were and still move a keysym, so the check
/// before each chunk compares these keycodes with the live keymap as well.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct KeyFingerprint {
    keys: Vec<(u8, Vec<u32>)>,
}

impl KeyFingerprint {
    pub(crate) fn record(&mut self, map: &Keymap, code: u8) {
        if self.keys.iter().all(|(known, _)| *known != code) {
            self.keys
                .push((code, map.keysyms_of(code).unwrap_or_default()));
        }
    }

    /// The first planned keycode whose keysyms differ in `live`.
    pub(crate) fn changed_in(&self, live: &Keymap) -> Option<u8> {
        self.keys
            .iter()
            .find(|(code, keysyms)| live.keysyms_of(*code).unwrap_or_default() != *keysyms)
            .map(|(code, _)| *code)
    }
}

#[cfg(test)]
#[path = "key_fingerprint_tests.rs"]
mod tests;
