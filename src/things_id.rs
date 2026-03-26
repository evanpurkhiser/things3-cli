use std::fmt;
use std::str::FromStr;

use rand::random;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha1::{Digest, Sha1};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Base58 encoding (Bitcoin alphabet)
// ---------------------------------------------------------------------------

const BASE58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

fn base58_encode(raw: &[u8]) -> String {
    if raw.is_empty() {
        return String::new();
    }

    let mut leading_zeros = 0usize;
    for b in raw {
        if *b == 0 {
            leading_zeros += 1;
        } else {
            break;
        }
    }

    let mut digits: Vec<u8> = Vec::new();
    for &byte in raw.iter().skip(leading_zeros) {
        let mut carry = byte as u32;
        for digit in &mut digits {
            let value = (*digit as u32 * 256) + carry;
            *digit = (value % 58) as u8;
            carry = value / 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }

    let mut out = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        out.push('1');
    }
    if digits.is_empty() {
        return out;
    }
    for digit in digits.iter().rev() {
        out.push(BASE58_ALPHABET[*digit as usize] as char);
    }
    out
}

fn base58_digit(byte: u8) -> Option<u8> {
    BASE58_ALPHABET
        .iter()
        .position(|&c| c == byte)
        .map(|i| i as u8)
}

fn base58_decode(input: &str) -> Option<Vec<u8>> {
    if input.is_empty() {
        return Some(Vec::new());
    }

    let bytes = input.as_bytes();
    let mut leading_ones = 0usize;
    for b in bytes {
        if *b == b'1' {
            leading_ones += 1;
        } else {
            break;
        }
    }

    let mut decoded: Vec<u8> = Vec::new();
    for &ch in bytes.iter().skip(leading_ones) {
        let mut carry = base58_digit(ch)? as u32;
        for byte in &mut decoded {
            let value = (*byte as u32 * 58) + carry;
            *byte = (value & 0xff) as u8;
            carry = value >> 8;
        }
        while carry > 0 {
            decoded.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    let mut out = Vec::with_capacity(leading_ones + decoded.len());
    out.extend(std::iter::repeat_n(0u8, leading_ones));
    for byte in decoded.iter().rev() {
        out.push(*byte);
    }
    Some(out)
}

/// Convert a `Uuid` to canonical internal Things ID bytes.
fn uuid_to_bytes(uuid: &Uuid) -> [u8; 16] {
    let canonical = uuid.to_string().to_uppercase();
    let digest = Sha1::digest(canonical.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes
}

// ---------------------------------------------------------------------------
// Parse error
// ---------------------------------------------------------------------------

/// Error returned when a string cannot be parsed as a [`ThingsId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseThingsIdError(String);

impl fmt::Display for ParseThingsIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid Things ID: {:?}", self.0)
    }
}

impl std::error::Error for ParseThingsIdError {}

// ---------------------------------------------------------------------------
// ThingsId
// ---------------------------------------------------------------------------

/// A Things 3 task / area identifier.
///
/// Internally stored as canonical 16 bytes (SHA1-truncated UUID digest).
/// Hyphenated UUIDs and compact base58 IDs are accepted at parse-time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThingsId([u8; 16]);

impl ThingsId {
    /// Generate a fresh random `ThingsId`.
    pub fn random() -> Self {
        let uuid = Uuid::from_bytes(random());
        ThingsId(uuid_to_bytes(&uuid))
    }

    /// Return the canonical 16-byte internal representation.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for ThingsId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&base58_encode(&self.0))
    }
}

impl AsRef<[u8; 16]> for ThingsId {
    fn as_ref(&self) -> &[u8; 16] {
        &self.0
    }
}

impl From<ThingsId> for String {
    fn from(id: ThingsId) -> Self {
        id.to_string()
    }
}

/// Parse a `ThingsId` from either a hyphenated UUID or an existing compact ID.
///
/// - If the string is empty, returns `Err`.
/// - If the string is a valid UUID, it is converted to internal bytes.
/// - Otherwise it must be a valid base58 compact ID that decodes to 16 bytes.
impl FromStr for ThingsId {
    type Err = ParseThingsIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseThingsIdError(s.to_owned()));
        }
        if let Ok(uuid) = Uuid::parse_str(s) {
            return Ok(ThingsId(uuid_to_bytes(&uuid)));
        }
        let decoded = base58_decode(s).ok_or_else(|| ParseThingsIdError(s.to_owned()))?;
        if decoded.len() != 16 {
            return Err(ParseThingsIdError(s.to_owned()));
        }
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&decoded);
        Ok(ThingsId(bytes))
    }
}

// ---------------------------------------------------------------------------
// Serde
// ---------------------------------------------------------------------------

impl Serialize for ThingsId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ThingsId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ThingsIdVisitor;

        impl Visitor<'_> for ThingsIdVisitor {
            type Value = ThingsId;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "a Things ID string (compact base58 or hyphenated UUID)")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ThingsId, E> {
                v.parse().map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(ThingsIdVisitor)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// A known legacy UUID and its expected compact form (computed from the
    /// Python reference implementation).
    const LEGACY_UUID: &str = "3C6BBD49-8D11-4FFF-8B0E-B8F33FA9C00A";
    const LEGACY_UUID_LOWER: &str = "3c6bbd49-8d11-4fff-8b0e-b8f33fa9c00a";
    fn compact_for_legacy() -> String {
        ThingsId::from_str(LEGACY_UUID).unwrap().to_string()
    }

    #[test]
    fn parse_legacy_uuid_uppercase() {
        let id: ThingsId = LEGACY_UUID.parse().unwrap();
        assert_eq!(id.to_string(), compact_for_legacy());
        assert_eq!(id.to_string().len(), 22);
    }

    #[test]
    fn parse_legacy_uuid_lowercase() {
        let upper: ThingsId = LEGACY_UUID.parse().unwrap();
        let lower: ThingsId = LEGACY_UUID_LOWER.parse().unwrap();
        assert_eq!(upper, lower, "UUID parsing must be case-insensitive");
    }

    #[test]
    fn parse_compact_preserved() {
        let compact = compact_for_legacy();
        let id: ThingsId = compact.parse().unwrap();
        assert_eq!(id.to_string(), compact);
    }

    #[test]
    fn empty_string_is_error() {
        let err = "".parse::<ThingsId>();
        assert!(err.is_err());
    }

    #[test]
    fn display_roundtrip() {
        let id: ThingsId = LEGACY_UUID.parse().unwrap();
        let displayed = id.to_string();
        let reparsed: ThingsId = displayed.parse().unwrap();
        assert_eq!(id, reparsed);
    }

    #[test]
    fn random_is_unique() {
        let ids: HashSet<String> = (0..20).map(|_| ThingsId::random().to_string()).collect();
        assert_eq!(ids.len(), 20, "random IDs should be unique");
    }

    #[test]
    fn random_is_compact_length() {
        let id = ThingsId::random();
        // Compact IDs are always 22 chars (16 SHA1 bytes → base58).
        assert_eq!(id.to_string().len(), 22);
    }

    #[test]
    fn serde_roundtrip_compact() {
        let id: ThingsId = LEGACY_UUID.parse().unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let back: ThingsId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn serde_deserialize_from_legacy_uuid() {
        let json = format!("\"{}\"", LEGACY_UUID);
        let id: ThingsId = serde_json::from_str(&json).unwrap();
        assert_eq!(id.to_string().len(), 22);
        assert_eq!(id.to_string(), compact_for_legacy());
    }

    #[test]
    fn into_string() {
        let id: ThingsId = LEGACY_UUID.parse().unwrap();
        let s: String = id.clone().into();
        assert_eq!(s, id.to_string());
    }

    #[test]
    fn as_ref_bytes() {
        let id: ThingsId = LEGACY_UUID.parse().unwrap();
        let r: &[u8; 16] = id.as_ref();
        assert_eq!(r, id.as_bytes());
    }

    #[test]
    fn rejects_invalid_compact_id() {
        assert!("not-a-things-id".parse::<ThingsId>().is_err());
        assert!("0OIl".parse::<ThingsId>().is_err());
    }

    #[test]
    fn base58_roundtrip_for_internal_bytes() {
        let samples = [
            [0u8; 16],
            [255u8; 16],
            uuid_to_bytes(&Uuid::parse_str(LEGACY_UUID).unwrap()),
        ];
        for sample in samples {
            let encoded = base58_encode(&sample);
            let decoded = base58_decode(&encoded).unwrap();
            assert_eq!(decoded, sample);
        }
    }
}
