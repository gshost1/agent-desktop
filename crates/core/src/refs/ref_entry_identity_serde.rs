//! Writes and reads a `RefEntryIdentity` key by key.
//!
//! On disk every entry carries `capture`, and a `Unique` one also carries the
//! older `unique_at_capture: true`, so a reader that predates the verdict keeps
//! resolving the refs it proved. Reading takes the verdict from `capture`; the
//! boolean is consulted only when `capture` is missing, which marks a refmap
//! written before the verdict existed, and a boolean that contradicts
//! `capture` reads as `Unknown`. A duplicate key is rejected, as the derived
//! reader did.

use std::fmt;

use serde::de::{Deserialize, Deserializer, Error, IgnoredAny, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};

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

impl Serialize for RefEntryIdentity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("RefEntryIdentity", FIELDS.len())?;
        optional(&mut out, "retained_object", &self.retained_object)?;
        out.serialize_field("role", &self.role)?;
        out.serialize_field("name", &self.name)?;
        optional(&mut out, "value", &self.value)?;
        optional(&mut out, "description", &self.description)?;
        optional(&mut out, "native_id", &self.native_id)?;
        out.serialize_field("capture", &self.capture)?;
        if self.capture.is_unique() {
            out.serialize_field("unique_at_capture", &true)?;
        } else {
            out.skip_field("unique_at_capture")?;
        }
        out.end()
    }
}

fn optional<S: SerializeStruct, T: Serialize>(
    out: &mut S,
    key: &'static str,
    value: &Option<T>,
) -> Result<(), S::Error> {
    match value {
        Some(value) => out.serialize_field(key, value),
        None => out.skip_field(key),
    }
}

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
        let mut retained_object: Option<Option<String>> = None;
        let mut role: Option<String> = None;
        let mut name: Option<Option<String>> = None;
        let mut value: Option<Option<String>> = None;
        let mut description: Option<Option<String>> = None;
        let mut native_id: Option<Option<ElementIdentifier>> = None;
        let mut capture: Option<CaptureVerdict> = None;
        let mut legacy: Option<bool> = None;
        while let Some(key) = map.next_key::<Key>()? {
            match key {
                Key::RetainedObject => once(&mut retained_object, &mut map, "retained_object")?,
                Key::Role => once(&mut role, &mut map, "role")?,
                Key::Name => once(&mut name, &mut map, "name")?,
                Key::Value => once(&mut value, &mut map, "value")?,
                Key::Description => once(&mut description, &mut map, "description")?,
                Key::NativeId => once(&mut native_id, &mut map, "native_id")?,
                Key::Capture => once(&mut capture, &mut map, "capture")?,
                Key::UniqueAtCapture => once(&mut legacy, &mut map, "unique_at_capture")?,
                Key::Other => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(RefEntryIdentity {
            retained_object: retained_object.flatten(),
            role: role.ok_or_else(|| A::Error::missing_field("role"))?,
            name: name.flatten(),
            value: value.flatten(),
            description: description.flatten(),
            native_id: native_id.flatten(),
            capture: verdict(capture, legacy),
        })
    }
}

/// The verdict an entry was written with. `capture` decides; a refmap without
/// it predates the verdict, where a missing flag meant `false`; a flag that
/// contradicts `capture` leaves the entry `Unknown` rather than failing the
/// whole refmap.
fn verdict(capture: Option<CaptureVerdict>, legacy: Option<bool>) -> CaptureVerdict {
    match (capture, legacy) {
        (Some(capture), Some(unique)) if capture.is_unique() != unique => CaptureVerdict::Unknown,
        (Some(capture), _) => capture,
        (None, legacy) => CaptureVerdict::from_legacy_flag(legacy.unwrap_or(false)),
    }
}

fn once<'de, T: Deserialize<'de>, A: MapAccess<'de>>(
    slot: &mut Option<T>,
    map: &mut A,
    field: &'static str,
) -> Result<(), A::Error> {
    if slot.is_some() {
        return Err(A::Error::duplicate_field(field));
    }
    *slot = Some(map.next_value()?);
    Ok(())
}

impl<'de> Deserialize<'de> for RefEntryIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("RefEntryIdentity", FIELDS, IdentityVisitor)
    }
}
