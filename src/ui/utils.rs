use crate::ids::ThingsId;

/// Returns the shortest displayable prefix for an ID, or empty when disabled.
pub(crate) fn id_prefix(id: &ThingsId, id_prefix_len: usize) -> String {
    if id_prefix_len == 0 {
        String::new()
    } else {
        id.to_string().chars().take(id_prefix_len).collect()
    }
}
