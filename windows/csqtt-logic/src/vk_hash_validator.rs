//! Port of VkHashValidator.kt.

use std::collections::{BTreeMap, HashSet};
use serde::{Deserialize, Serialize};

/// Validation status of a VK hash candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VkHashValidationStatus {
    #[serde(rename = "valid")]
    Valid,
    #[serde(rename = "invalid")]
    Invalid,
}

impl VkHashValidationStatus {
    /// Wire value string for serialization ("valid" or "invalid").
    pub const fn wire_value(&self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
        }
    }

    /// Deserializes status from wire value string.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "valid" => Some(Self::Valid),
            "invalid" => Some(Self::Invalid),
            _ => None,
        }
    }
}

/// Codec and filtering utilities for persisted VK hash validation states.
pub struct VkHashValidationCodec;

impl VkHashValidationCodec {
    /// Decodes a JSON map of hash to status wire values. Returns empty map on invalid format.
    pub fn decode(value: &str) -> BTreeMap<String, VkHashValidationStatus> {
        decode_results(value)
    }

    /// Encodes a map of hash to status into sorted-key JSON.
    pub fn encode(results: &BTreeMap<String, VkHashValidationStatus>) -> String {
        encode_results(results)
    }

    /// Returns distinct candidate hashes (len >= 16) not yet present in validation results.
    pub fn pending(
        hashes: &[impl AsRef<str>],
        results: &BTreeMap<String, VkHashValidationStatus>,
    ) -> Vec<String> {
        filter_pending_hashes(hashes, results)
    }

    /// Returns distinct trimmed candidate hashes (len >= 16) whose status is not `Invalid`.
    pub fn active(
        hashes: &[impl AsRef<str>],
        results: &BTreeMap<String, VkHashValidationStatus>,
    ) -> Vec<String> {
        filter_active_hashes(hashes, results)
    }

    /// Removes a previously validated hash from the results map.
    pub fn invalidate(
        results: &BTreeMap<String, VkHashValidationStatus>,
        previous_hash: &str,
    ) -> BTreeMap<String, VkHashValidationStatus> {
        invalidate_hash(results, previous_hash)
    }
}

/// Decodes validation results from JSON string.
pub fn decode_results(value: &str) -> BTreeMap<String, VkHashValidationStatus> {
    let parsed: serde_json::Value = match serde_json::from_str(value) {
        Ok(v) => v,
        Err(_) => return BTreeMap::new(),
    };
    let obj = match parsed.as_object() {
        Some(o) => o,
        None => return BTreeMap::new(),
    };
    let mut map = BTreeMap::new();
    for (hash, status_val) in obj {
        if let Some(status_str) = status_val.as_str() {
            if let Some(status) = VkHashValidationStatus::from_wire(status_str) {
                map.insert(hash.clone(), status);
            }
        }
    }
    map
}

/// Encodes validation results to JSON with sorted keys.
pub fn encode_results(results: &BTreeMap<String, VkHashValidationStatus>) -> String {
    let mut map = serde_json::Map::new();
    for (hash, status) in results {
        map.insert(
            hash.clone(),
            serde_json::Value::String(status.wire_value().to_string()),
        );
    }
    serde_json::Value::Object(map).to_string()
}

/// Filters candidate hashes (len >= 16) not present in results.
pub fn filter_pending_hashes(
    hashes: &[impl AsRef<str>],
    results: &BTreeMap<String, VkHashValidationStatus>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for h in hashes {
        let s = h.as_ref();
        if s.len() >= 16 && seen.insert(s.to_string()) && !results.contains_key(s) {
            out.push(s.to_string());
        }
    }
    out
}

/// Filters candidate hashes (len >= 16) whose status is not `Invalid`.
pub fn filter_active_hashes(
    hashes: &[impl AsRef<str>],
    results: &BTreeMap<String, VkHashValidationStatus>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for h in hashes {
        let trimmed = h.as_ref().trim();
        if trimmed.len() >= 16
            && seen.insert(trimmed.to_string())
            && results.get(trimmed) != Some(&VkHashValidationStatus::Invalid)
        {
            out.push(trimmed.to_string());
        }
    }
    out
}

/// Invalidates a hash entry from validation results.
pub fn invalidate_hash(
    results: &BTreeMap<String, VkHashValidationStatus>,
    previous_hash: &str,
) -> BTreeMap<String, VkHashValidationStatus> {
    let mut copy = results.clone();
    copy.remove(previous_hash);
    copy
}

/// Line output prefix used by validator binary.
pub const OUTPUT_PREFIX: &str = "HASH_CHECK:";

/// Validator command builder and output parser.
pub struct VkHashValidator;

impl VkHashValidator {
    pub const OUTPUT_PREFIX: &str = OUTPUT_PREFIX;

    /// Builds command arguments for launching the native hash validator.
    pub fn build_command(
        binary: &str,
        hashes: &[impl AsRef<str>],
        fingerprint: &str,
        client_ids: &str,
    ) -> Vec<String> {
        build_command(binary, hashes, fingerprint, client_ids)
    }

    /// Parses an output line emitted by the validator binary.
    pub fn parse_output_line(line: &str) -> Option<(String, VkHashValidationStatus)> {
        parse_output_line(line)
    }
}

/// Builds command arguments for launching the native hash validator.
pub fn build_command(
    binary: &str,
    hashes: &[impl AsRef<str>],
    fingerprint: &str,
    client_ids: &str,
) -> Vec<String> {
    let joined_hashes = hashes
        .iter()
        .map(|h| h.as_ref())
        .collect::<Vec<_>>()
        .join(",");
    vec![
        binary.to_string(),
        "--validate-vk-hashes".to_string(),
        "--vk".to_string(),
        joined_hashes,
        "--fingerprint".to_string(),
        fingerprint.to_string(),
        "--client-ids".to_string(),
        client_ids.to_string(),
    ]
}

/// Parses a stdout line from the validator binary prefixed with `HASH_CHECK:`.
pub fn parse_output_line(line: &str) -> Option<(String, VkHashValidationStatus)> {
    let payload = line.strip_prefix(OUTPUT_PREFIX)?;
    let parsed: serde_json::Value = serde_json::from_str(payload).ok()?;
    let obj = parsed.as_object()?;
    let hash = obj.get("hash")?.as_str()?;
    let status_str = obj.get("status")?.as_str()?;
    let status = VkHashValidationStatus::from_wire(status_str)?;
    Some((hash.to_string(), status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_results_round_trip_through_persistent_json() {
        let mut source = BTreeMap::new();
        source.insert("valid-hash-123456".to_string(), VkHashValidationStatus::Valid);
        source.insert("invalid-hash-1234".to_string(), VkHashValidationStatus::Invalid);
        assert_eq!(
            source,
            VkHashValidationCodec::decode(&VkHashValidationCodec::encode(&source)),
        );
    }

    #[test]
    fn only_changed_or_unchecked_hashes_are_pending() {
        let valid = "valid-hash-123456";
        let invalid = "invalid-hash-1234";
        let replacement = "replacement-123456";
        let mut checked = BTreeMap::new();
        checked.insert(valid.to_string(), VkHashValidationStatus::Valid);
        checked.insert(invalid.to_string(), VkHashValidationStatus::Invalid);
        let after_edit = VkHashValidationCodec::invalidate(&checked, invalid);
        assert_eq!(
            vec![replacement.to_string()],
            VkHashValidationCodec::pending(&[valid, replacement], &after_edit),
        );
        assert!(after_edit.contains_key(valid));
        assert!(!after_edit.contains_key(invalid));
    }

    #[test]
    fn invalid_hash_is_retained_for_ui_but_excluded_from_tunnel_source() {
        let active = "active-hash-123456";
        let invalid = "invalid-hash-1234";
        let mut results = BTreeMap::new();
        results.insert(invalid.to_string(), VkHashValidationStatus::Invalid);

        assert_eq!(
            vec![active.to_string()],
            VkHashValidationCodec::active(&[active, invalid, active], &results),
        );
        assert_eq!(Some(&VkHashValidationStatus::Invalid), results.get(invalid));
    }

    #[test]
    fn process_output_accepts_only_final_valid_or_invalid_states() {
        let valid = VkHashValidator::parse_output_line(
            "HASH_CHECK:{\"hash\":\"hash-123456789012\",\"status\":\"valid\"}",
        );
        let invalid = VkHashValidator::parse_output_line(
            "HASH_CHECK:{\"hash\":\"hash-210987654321\",\"status\":\"invalid\",\"code\":9008}",
        );
        assert_eq!(Some(VkHashValidationStatus::Valid), valid.as_ref().map(|p| p.1));
        assert_eq!(Some(VkHashValidationStatus::Invalid), invalid.as_ref().map(|p| p.1));
        assert_eq!(
            None,
            VkHashValidator::parse_output_line(
                "HASH_CHECK:{\"hash\":\"hash-123456789012\",\"status\":\"unavailable\"}",
            ),
        );
    }

    #[test]
    fn validator_mode_does_not_contain_tunnel_or_turn_arguments() {
        let command = VkHashValidator::build_command(
            "/native/libclient.so",
            &["hash-123456789012"],
            "firefox",
            "8202606,6287487",
        );
        assert!(command.iter().any(|c| c == "--validate-vk-hashes"));
        assert!(command.iter().any(|c| c == "--vk"));
        assert!(!command.iter().any(|c| c == "--peer"));
        assert!(!command.iter().any(|c| c == "--password"));
        assert!(!command.iter().any(|c| c == "--workers"));
    }
}
