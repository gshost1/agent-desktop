/// Which matching menu item, if any, a chord may press. Disabled matches are
/// inert to the keyboard as well, so only enabled ones count toward a choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuShortcutChoice {
    Press(usize),
    Ambiguous,
    Disabled,
    NoMatch,
}

impl MenuShortcutChoice {
    pub(crate) fn choose(enabled: &[bool]) -> Self {
        let mut pressable = enabled
            .iter()
            .enumerate()
            .filter(|(_, enabled)| **enabled)
            .map(|(index, _)| index);
        match (pressable.next(), pressable.next()) {
            (Some(index), None) => Self::Press(index),
            (Some(_), Some(_)) => Self::Ambiguous,
            (None, _) if enabled.is_empty() => Self::NoMatch,
            (None, _) => Self::Disabled,
        }
    }
}

#[cfg(test)]
#[path = "menu_shortcut_choice_tests.rs"]
mod tests;
