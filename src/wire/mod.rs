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

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

pub(crate) fn deserialize_optional_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

pub(crate) fn deserialize_default_on_null<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

pub(crate) fn deserialize_vec_or_single<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<OneOrMany<T>>::deserialize(deserializer).map(|value| match value {
        None => Vec::new(),
        Some(OneOrMany::One(v)) => vec![v],
        Some(OneOrMany::Many(v)) => v,
    })
}
