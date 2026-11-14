use agent_desktop_core::{KeyCombo, Modifier};

const BLOCKED: &[(&[Modifier], &str)] = &[
    (&[Modifier::Meta], "q"),
    (&[Modifier::Meta, Modifier::Alt], "esc"),
    (&[Modifier::Meta, Modifier::Shift], "delete"),
];

/// Reports whether `combo` is one of the macOS shortcuts that would quit, log
/// out, force-quit, or lock the session.
///
/// A combo is blocked if its key matches a listed shortcut and its modifiers
/// are a superset of that shortcut's modifiers. Adding a modifier to a
/// dangerous shortcut usually produces another dangerous shortcut rather than
/// a harmless one: `cmd+shift+q` force-quits and `ctrl+cmd+q` locks the screen
/// like `cmd+q` quits. Superset matching catches these cases and over-blocks a
/// few combinations nobody uses, which trades a wrong-block for a correct deny
/// and costs the caller one `--force`.
pub(crate) fn is_blocked(combo: &KeyCombo) -> bool {
    let lower = combo.key.to_ascii_lowercase();
    let key = canonical_key(&lower);
    BLOCKED.iter().any(|(needed, blocked_key)| {
        *blocked_key == key
            && needed
                .iter()
                .all(|modifier| combo.modifiers.contains(modifier))
    })
}

fn canonical_key(key: &str) -> &str {
    match key {
        "escape" | "esc" => "esc",
        "backspace" | "delete" => "delete",
        "enter" | "return" => "return",
        other => other,
    }
}

#[cfg(test)]
#[path = "blocked_combo_tests.rs"]
mod tests;
