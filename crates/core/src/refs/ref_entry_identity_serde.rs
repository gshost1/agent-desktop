//! Reads a `RefEntryIdentity` key by key, so the capture verdict accepts only
//! its names under `capture` and an older refmap's boolean only under
//! `unique_at_capture`, without buffering any value of the entry.

use std::fmt;

use serde::de::{Deserialize, Deserializer, Error, IgnoredAny, MapAccess, Visitor};

use super::RefEntryIdentity;
use crate::{CaptureVerdict, ElementIdentifier};

const FIELDS: &[&str] = &[
    "retained_object",
    "role",
    "name",
    "value",
    "description",
    "native_id",
    "capture",
    "unique_at_capture",
];

#[derive(serde::Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Key {
    RetainedObject,
    Role,
    Name,
    Value,
    Description,
    NativeId,
    Capture,
    UniqueAtCapture,
    #[serde(other)]
    Other,
}

struct IdentityVisitor;

impl<'de> Visitor<'de> for IdentityVisitor {
    type Value = RefEntryIdentity;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a ref entry identity")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RefEntryIdentity, A::Error> {
        let mut identity = RefEntryIdentity::default();
        let mut role = None;
        let mut capture: Option<CaptureVerdict> = None;
        let mut legacy: Option<bool> = None;
        while let Some(key) = map.next_key::<Key>()? {
            match key {
                Key::RetainedObject => identity.retained_object = map.next_value()?,
                Key::Role => set_once(&mut role, map.next_value::<String>()?, "role")?,
                Key::Name => identity.name = map.next_value()?,
                Key::Value => identity.value = map.next_value()?,
                Key::Description => identity.description = map.next_value()?,
                Key::NativeId => {
                    identity.native_id = map.next_value::<Option<ElementIdentifier>>()?
                }
                Key::Capture => set_once(&mut capture, map.next_value()?, "capture")?,
                Key::UniqueAtCapture => {
                    set_once(&mut legacy, map.next_value()?, "unique_at_capture")?
                }
                Key::Other => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        identity.role = role.ok_or_else(|| A::Error::missing_field("role"))?;
        identity.capture =
            capture.unwrap_or_else(|| CaptureVerdict::from_legacy_flag(legacy.unwrap_or(false)));
        Ok(identity)
    }
}

fn set_once<T, E: Error>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E> {
    if slot.is_some() {
        return Err(E::duplicate_field(field));
    }
    *slot = Some(value);
    Ok(())
}

impl<'de> Deserialize<'de> for RefEntryIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("RefEntryIdentity", FIELDS, IdentityVisitor)
    }
}
