/// A key `Input.dispatchKeyEvent` can press: its DOM `key` and `code`, its
/// Windows virtual key code, and the text it types, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyDefinition {
    pub(crate) key: &'static str,
    pub(crate) code: &'static str,
    pub(crate) key_code: u32,
    pub(crate) text: Option<&'static str>,
}

const fn key(key: &'static str, key_code: u32, text: Option<&'static str>) -> KeyDefinition {
    KeyDefinition {
        key,
        code: key,
        key_code,
        text,
    }
}

pub(crate) const KEYS: &[KeyDefinition] = &[
    key("Enter", 13, Some("\r")),
    key("Tab", 9, None),
    key("Escape", 27, None),
    key("Backspace", 8, None),
    key("Delete", 46, None),
    key("ArrowUp", 38, None),
    key("ArrowDown", 40, None),
    key("ArrowLeft", 37, None),
    key("ArrowRight", 39, None),
    key("Home", 36, None),
    key("End", 35, None),
    key("PageUp", 33, None),
    key("PageDown", 34, None),
    KeyDefinition {
        key: " ",
        code: "Space",
        key_code: 32,
        text: Some(" "),
    },
];

/// Finds a key by its DOM name or a common alias, ignoring case.
pub(crate) fn find_key(name: &str) -> Option<KeyDefinition> {
    let lowered = name.trim().to_ascii_lowercase();
    let canonical = match lowered.as_str() {
        "return" => "enter",
        "esc" => "escape",
        "up" => "arrowup",
        "down" => "arrowdown",
        "left" => "arrowleft",
        "right" => "arrowright",
        "del" => "delete",
        "space" | "spacebar" => " ",
        other => other,
    };
    KEYS.iter()
        .copied()
        .find(|definition| definition.key.eq_ignore_ascii_case(canonical))
}
