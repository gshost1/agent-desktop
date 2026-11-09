/// The key half of a menu shortcut. A printable key is matched against the
/// character a menu item publishes; a named key such as `delete` or `f5` has no
/// printable character and is matched against the item's virtual key code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShortcutKey {
    Character(String),
    VirtualKey(u16),
}
