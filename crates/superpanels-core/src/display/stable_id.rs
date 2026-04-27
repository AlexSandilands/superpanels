//! Helpers for synthesising a `MonitorRef::stable_id` on detectors that
//! don't emit one directly.
//!
//! `SPEC.md` §6.4 documents the rule: hash `make + "\0" + model + "\0" +
//! serial` with SHA-256, encode as lowercase hex. Cable swaps and dock
//! re-plugs change `name` but leave the EDID-derived triple intact, so
//! per-monitor config keyed on the hash survives both.

use sha2::{Digest, Sha256};

/// Build a `stable_id` from the EDID `make + model + serial` triple.
///
/// Returns `None` when none of the three fields are present — the caller
/// should fall back to the monitor's `name` per `SPEC.md` §6.4. Empty
/// strings are treated the same as `None` so a detector that emits
/// `"serial": ""` doesn't poison the hash.
pub(crate) fn hash_edid_triple(
    make: Option<&str>,
    model: Option<&str>,
    serial: Option<&str>,
) -> Option<String> {
    let make = nonempty(make);
    let model = nonempty(model);
    let serial = nonempty(serial);
    if make.is_none() && model.is_none() && serial.is_none() {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(make.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(model.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(serial.unwrap_or("").as_bytes());
    Some(hex_lower(&hasher.finalize()))
}

fn nonempty(s: Option<&str>) -> Option<&str> {
    s.and_then(|v| {
        let trimmed = v.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on hash construction errors
mod tests {
    use super::*;

    #[test]
    fn all_fields_present_produces_64_char_hex_digest() {
        let id = hash_edid_triple(Some("Dell"), Some("U2723QE"), Some("ABC123")).unwrap();
        assert_eq!(id.len(), 64);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
        );
    }

    #[test]
    fn different_inputs_produce_different_digests() {
        let a = hash_edid_triple(Some("Dell"), Some("U2723QE"), Some("ABC123")).unwrap();
        let b = hash_edid_triple(Some("Dell"), Some("U2723QE"), Some("DEF456")).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn all_none_returns_none() {
        assert_eq!(hash_edid_triple(None, None, None), None);
    }

    #[test]
    fn empty_strings_treated_as_missing() {
        assert_eq!(hash_edid_triple(Some(""), Some("  "), Some("")), None);
    }

    #[test]
    fn delimiters_prevent_field_collision() {
        // Without the null delimiter, ("ab", "c") and ("a", "bc") would
        // collide. The delimiter keeps them distinct.
        let a = hash_edid_triple(Some("ab"), Some("c"), Some("x")).unwrap();
        let b = hash_edid_triple(Some("a"), Some("bc"), Some("x")).unwrap();
        assert_ne!(a, b);
    }
}
