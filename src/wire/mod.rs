//! Things Cloud sync protocol wire-format types.
//!
//! Observed item shape in history pages:
//! `{ uuid: { "t": operation, "e": entity, "p": properties } }`.
//! Replaying items in order by UUID yields current state.

use serde::{Deserialize, Deserializer};

pub mod area;
pub mod checklist;
pub mod notes;
pub mod recurrence;
pub mod tags;
pub mod task;
pub mod tombstone;
pub mod wire_object;

pub(crate) fn deserialize_optional_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
