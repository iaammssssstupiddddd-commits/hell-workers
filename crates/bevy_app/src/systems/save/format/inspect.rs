//! Bounded header inspection for catalog status without DynamicWorld deserialize.

use super::{
    DecodedSaveFile, SAVE_MAGIC, SaveFormat, SaveFormatError, SaveHeader, decode_save_file,
};

/// Catalog / revision fingerprint reads never exceed this prefix.
pub const SAVE_HEADER_INSPECT_LIMIT_BYTES: usize = 16 * 1024;

/// Content classification derived from a bounded header prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveHeaderStatus {
    /// File is missing or zero-length.
    Empty,
    /// Current v1 header with a readable worldgen seed.
    CurrentV1 { worldgen_seed: u64 },
    /// Magic-less bytes that may be a legacy v0 body. Never asserted as valid v0.
    LegacyV0Candidate,
    /// v1 header seed does not match the live worldgen seed.
    SeedMismatch { worldgen_seed: u64 },
    /// Declared format version is not the current container version.
    UnsupportedVersion { found: u32 },
    /// Magic present but header/separator is truncated or malformed within the limit.
    CorruptHeader,
    /// Filesystem read of the existing target failed.
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectedSavePrefix<'a> {
    pub bytes_read: usize,
    pub status: SaveHeaderStatus,
    /// UTF-8 lossy view used only for decode; DynamicWorld body is never parsed.
    pub prefix: &'a str,
}

/// Classifies a bounded UTF-8 prefix. The DynamicWorld body is never deserialized.
pub fn inspect_save_prefix(
    prefix: &str,
    bytes_read: usize,
    expected_worldgen_seed: Option<u64>,
) -> InspectedSavePrefix<'_> {
    if prefix.is_empty() {
        return InspectedSavePrefix {
            bytes_read,
            status: SaveHeaderStatus::Empty,
            prefix,
        };
    }

    if prefix.starts_with(SAVE_MAGIC) {
        // Truncation inside the inspect window without a separator is corrupt.
        if !prefix.contains("\n---\n") && !prefix.contains("\r\n---\r\n") {
            return InspectedSavePrefix {
                bytes_read,
                status: SaveHeaderStatus::CorruptHeader,
                prefix,
            };
        }
    }

    match decode_save_file(prefix) {
        Ok(DecodedSaveFile {
            format: SaveFormat::LegacyV0,
            ..
        }) => InspectedSavePrefix {
            bytes_read,
            status: SaveHeaderStatus::LegacyV0Candidate,
            prefix,
        },
        Ok(DecodedSaveFile {
            format:
                SaveFormat::V1(SaveHeader {
                    format_version: _,
                    worldgen_seed,
                }),
            ..
        }) => {
            let status = match expected_worldgen_seed {
                Some(expected) if expected != worldgen_seed => {
                    SaveHeaderStatus::SeedMismatch { worldgen_seed }
                }
                _ => SaveHeaderStatus::CurrentV1 { worldgen_seed },
            };
            InspectedSavePrefix {
                bytes_read,
                status,
                prefix,
            }
        }
        Err(SaveFormatError::UnsupportedVersion { found, .. }) => InspectedSavePrefix {
            bytes_read,
            status: SaveHeaderStatus::UnsupportedVersion { found },
            prefix,
        },
        Err(
            SaveFormatError::MissingHeaderLineBreak
            | SaveFormatError::MissingBodySeparator
            | SaveFormatError::InvalidHeader(_),
        ) => InspectedSavePrefix {
            bytes_read,
            status: SaveHeaderStatus::CorruptHeader,
            prefix,
        },
    }
}

/// Player-safe status label. Never includes paths or OS errors.
pub fn save_header_status_label(status: SaveHeaderStatus) -> &'static str {
    match status {
        SaveHeaderStatus::Empty => "Empty",
        SaveHeaderStatus::CurrentV1 { .. } => "Current format",
        SaveHeaderStatus::LegacyV0Candidate => "Legacy save (validation required)",
        SaveHeaderStatus::SeedMismatch { .. } => "Different world seed",
        SaveHeaderStatus::UnsupportedVersion { .. } => "Newer/unsupported version",
        SaveHeaderStatus::CorruptHeader => "Corrupt save header",
        SaveHeaderStatus::Unreadable => "Save file unavailable",
    }
}

pub const fn save_header_status_allows_load(status: SaveHeaderStatus) -> bool {
    matches!(
        status,
        SaveHeaderStatus::CurrentV1 { .. } | SaveHeaderStatus::LegacyV0Candidate
    )
}

#[cfg(test)]
mod inspect_tests {
    use super::*;
    use crate::systems::save::format::{CURRENT_SAVE_FORMAT_VERSION, SaveHeader, encode_save_file};

    #[test]
    fn inspect_classifies_current_v1_without_body_parse() {
        let encoded = encode_save_file(SaveHeader::current(99), "not a DynamicWorld body");
        let inspected = inspect_save_prefix(&encoded, encoded.len(), Some(99));
        assert_eq!(
            inspected.status,
            SaveHeaderStatus::CurrentV1 { worldgen_seed: 99 }
        );
        assert!(save_header_status_allows_load(inspected.status));
    }

    #[test]
    fn magicless_bytes_are_legacy_candidates_not_valid_v0() {
        let inspected = inspect_save_prefix("plain ron-ish bytes", 18, Some(1));
        assert_eq!(inspected.status, SaveHeaderStatus::LegacyV0Candidate);
        assert_eq!(
            save_header_status_label(inspected.status),
            "Legacy save (validation required)"
        );
    }

    #[test]
    fn seed_mismatch_and_future_version_are_load_disabled() {
        let encoded = encode_save_file(SaveHeader::current(7), "body");
        let mismatch = inspect_save_prefix(&encoded, encoded.len(), Some(8));
        assert_eq!(
            mismatch.status,
            SaveHeaderStatus::SeedMismatch { worldgen_seed: 7 }
        );
        assert!(!save_header_status_allows_load(mismatch.status));

        let future = format!(
            "{SAVE_MAGIC}\n(format_version: {}, worldgen_seed: 1)\n---\nbody",
            CURRENT_SAVE_FORMAT_VERSION + 3
        );
        let unsupported = inspect_save_prefix(&future, future.len(), Some(1));
        assert_eq!(
            unsupported.status,
            SaveHeaderStatus::UnsupportedVersion {
                found: CURRENT_SAVE_FORMAT_VERSION + 3
            }
        );
        assert!(!save_header_status_allows_load(unsupported.status));
    }

    #[test]
    fn truncated_magic_prefix_is_corrupt_not_legacy() {
        let truncated = format!("{SAVE_MAGIC}\n(format_version: 1, worldgen_seed: 1)\n");
        let inspected = inspect_save_prefix(&truncated, truncated.len(), Some(1));
        assert_eq!(inspected.status, SaveHeaderStatus::CorruptHeader);
    }

    #[test]
    fn empty_prefix_is_empty_status() {
        let inspected = inspect_save_prefix("", 0, Some(1));
        assert_eq!(inspected.status, SaveHeaderStatus::Empty);
        assert!(!save_header_status_allows_load(inspected.status));
    }
}
