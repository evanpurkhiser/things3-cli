//! Thin compatibility wrappers around [`crate::things_id::ThingsId`].
//!
//! New code should use `ThingsId` directly.  These functions exist to avoid
//! a large call-site churn in the initial refactor.

use crate::things_id::ThingsId;

/// Convert a hyphenated UUID string to a compact Things ID.
///
/// Returns `None` if `value` is not a valid UUID (i.e. it is already compact).
pub fn legacy_uuid_to_task_id(value: &str) -> Option<String> {
    // `ThingsId::from_str` accepts both forms.  We return `None` when the
    // input is already compact (not a UUID) so callers can use
    // `legacy_uuid_to_task_id(&s).unwrap_or(s)` as before.
    use std::str::FromStr;
    use uuid::Uuid;
    if Uuid::parse_str(value).is_err() {
        return None;
    }
    Some(ThingsId::from_str(value).ok()?.into())
}

/// Generate a new random Things ID.
pub fn random_task_id() -> String {
    ThingsId::random().into()
}
