use crate::ids::things_id::base58_encode_fixed;
use crate::ids::ThingsId;
use std::collections::HashMap;

pub fn lcp_len(a: &str, b: &str) -> usize {
    lcp_len_bytes(a.as_bytes(), b.as_bytes())
}

/// Compute the shortest unique prefix for every ID.
///
/// Encodes each ID exactly once into a stack-allocated `[u8; 22]` buffer.
/// The sort and LCP scan operate on byte slices with no heap allocation;
/// only the final `result.insert` allocates a `String` per entry.
pub fn shortest_unique_prefixes(ids: &[ThingsId]) -> HashMap<ThingsId, String> {
    if ids.is_empty() {
        return HashMap::new();
    }

    // Encode each ID once into a fixed stack buffer + length.
    let mut pairs: Vec<(ThingsId, [u8; 22], usize)> = ids
        .iter()
        .map(|id| {
            let (buf, len) = base58_encode_fixed(id.as_bytes());
            (id.clone(), buf, len)
        })
        .collect();
    // Sort by the encoded string slice (same order as to_string() comparisons).
    pairs.sort_unstable_by(|a, b| a.1[..a.2].cmp(&b.1[..b.2]));

    let n = pairs.len();
    let mut result = HashMap::with_capacity(n);
    for i in 0..n {
        let enc = &pairs[i].1[..pairs[i].2];
        let left = if i > 0 {
            lcp_len_bytes(enc, &pairs[i - 1].1[..pairs[i - 1].2])
        } else {
            0
        };
        let right = if i + 1 < n {
            lcp_len_bytes(enc, &pairs[i + 1].1[..pairs[i + 1].2])
        } else {
            0
        };
        // +1 to go one character beyond the shared prefix.
        // Clamp to the full encoded length — if an ID's encoding is a
        // prefix of another, the full string is the shortest unique prefix.
        let need = (left.max(right) + 1).min(pairs[i].2);
        // SAFETY: base58 output is pure ASCII so any byte prefix is valid UTF-8.
        let prefix = unsafe { std::str::from_utf8_unchecked(&enc[..need]) }.to_owned();
        result.insert(pairs[i].0.clone(), prefix);
    }

    result
}

/// Return the shared width needed to display group-local unique IDs.
///
/// This is the maximum length among all shortest unique prefixes for the
/// given group. Callers can then render `id[..width]` for every row.
pub fn longest_shortest_unique_prefix_len(ids: &[ThingsId]) -> usize {
    if ids.is_empty() {
        return 0;
    }

    shortest_unique_prefixes(ids)
        .values()
        .map(|s| s.len())
        .max()
        .unwrap_or(1)
}

#[inline]
fn lcp_len_bytes(a: &[u8], b: &[u8]) -> usize {
    let max = a.len().min(b.len());
    for i in 0..max {
        if a[i] != b[i] {
            return i;
        }
    }
    max
}

pub fn prefix_matches<'a>(sorted_ids: &'a [ThingsId], prefix: &str) -> Vec<&'a ThingsId> {
    sorted_ids
        .iter()
        .filter(|id| id.starts_with(prefix))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_unique_prefixes_are_actually_unique() {
        // Generate a set of random IDs and verify that each prefix matches
        // exactly one ID from the input set.
        let ids: Vec<ThingsId> = (0..200).map(|_| ThingsId::random()).collect();
        let prefixes = shortest_unique_prefixes(&ids);

        assert_eq!(prefixes.len(), ids.len());

        for (id, prefix) in &prefixes {
            let matches: Vec<&ThingsId> = ids
                .iter()
                .filter(|other| other.to_string().starts_with(prefix.as_str()))
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "prefix {:?} for {:?} matched {} IDs: {:?}",
                prefix,
                id.to_string(),
                matches.len(),
                matches.iter().map(|m| m.to_string()).collect::<Vec<_>>()
            );
            assert_eq!(matches[0], id);
        }
    }

    #[test]
    fn shortest_unique_prefixes_are_minimal() {
        // Each prefix should be the shortest possible: removing the last
        // character should make it match more than one ID.
        let ids: Vec<ThingsId> = (0..200).map(|_| ThingsId::random()).collect();
        let prefixes = shortest_unique_prefixes(&ids);

        for (_id, prefix) in &prefixes {
            if prefix.len() <= 1 {
                continue; // can't shorten a 1-char prefix
            }
            let shorter = &prefix[..prefix.len() - 1];
            let matches: Vec<&ThingsId> = ids
                .iter()
                .filter(|other| other.to_string().starts_with(shorter))
                .collect();
            assert!(
                matches.len() > 1,
                "prefix {:?} is not minimal — {:?} (one char shorter) still only matches 1 ID",
                prefix,
                shorter
            );
        }
    }

    #[test]
    fn single_id() {
        let ids = vec![ThingsId::random()];
        let prefixes = shortest_unique_prefixes(&ids);
        assert_eq!(prefixes.len(), 1);
        let prefix = prefixes.values().next().unwrap();
        assert_eq!(prefix.len(), 1, "single ID should have 1-char prefix");
    }

    #[test]
    fn empty_input() {
        let prefixes = shortest_unique_prefixes(&[]);
        assert!(prefixes.is_empty());
    }

    #[test]
    fn longest_shortest_prefix_len_drives_fixed_width_unique_rendering() {
        fn find_case_ids() -> Vec<ThingsId> {
            let mut a: Option<ThingsId> = None;
            let mut b_left: Option<ThingsId> = None;
            let mut b_right: Option<ThingsId> = None;

            for n in 1u64..200_000 {
                let mut raw = [0u8; 16];
                for (i, byte) in raw.iter_mut().enumerate() {
                    let shift = ((i % 8) * 8) as u32;
                    let mixed = n.wrapping_mul(0x9E37_79B9_7F4A_7C15u64 ^ (i as u64 * 0xA5A5u64));
                    *byte = ((mixed >> shift) & 0xFF) as u8;
                }

                let (buf, len) = base58_encode_fixed(&raw);
                let s = String::from_utf8(buf[..len].to_vec()).unwrap();
                let id = s.parse::<ThingsId>().unwrap();

                if s.starts_with('A') {
                    if a.is_none() {
                        a = Some(id);
                    }
                    continue;
                }

                if s.starts_with('B') {
                    let second = s.chars().nth(1).unwrap_or('1');
                    if b_left.is_none() {
                        b_left = Some(id);
                    } else {
                        let left_second = b_left
                            .as_ref()
                            .unwrap()
                            .to_string()
                            .chars()
                            .nth(1)
                            .unwrap_or('1');
                        if second != left_second {
                            b_right = Some(id);
                        }
                    }
                }

                if a.is_some() && b_left.is_some() && b_right.is_some() {
                    break;
                }
            }

            vec![
                a.expect("must find an A* id"),
                b_left.expect("must find first B* id"),
                b_right.expect("must find second B* id with different second char"),
            ]
        }

        let ids = find_case_ids();
        let width = longest_shortest_unique_prefix_len(&ids);
        assert_eq!(width, 2);

        for id in &ids {
            let prefix = id.to_string().chars().take(width).collect::<String>();
            let matches: Vec<&ThingsId> = ids
                .iter()
                .filter(|other| other.to_string().starts_with(prefix.as_str()))
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "prefix {prefix:?} should identify exactly one id"
            );
            assert_eq!(matches[0], id);
        }
    }
}
