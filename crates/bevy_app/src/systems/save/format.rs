//! DynamicWorld body の外側に置く、registry 非依存のセーブ形式ヘッダー。

use std::fmt;

use serde::{Deserialize, Serialize};

pub const SAVE_MAGIC: &str = "HELL_WORKERS_SAVE";
pub const CURRENT_SAVE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveHeader {
    pub format_version: u32,
    pub worldgen_seed: u64,
}

impl SaveHeader {
    pub const fn current(worldgen_seed: u64) -> Self {
        Self {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            worldgen_seed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    /// Header 導入前の DynamicWorld RON。seed は body 内の legacy Resource から読む。
    LegacyV0,
    V1(SaveHeader),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSaveFile<'a> {
    pub format: SaveFormat,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveFormatError {
    MissingHeaderLineBreak,
    MissingBodySeparator,
    InvalidHeader(String),
    UnsupportedVersion { found: u32, current: u32 },
}

impl fmt::Display for SaveFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeaderLineBreak => {
                formatter.write_str("save magic is not followed by a header line")
            }
            Self::MissingBodySeparator => formatter.write_str("save header has no body separator"),
            Self::InvalidHeader(error) => write!(formatter, "invalid save header: {error}"),
            Self::UnsupportedVersion { found, current } => write!(
                formatter,
                "unsupported save format version {found} (current version is {current})"
            ),
        }
    }
}

impl std::error::Error for SaveFormatError {}

/// Encodes a v1 file without involving the DynamicWorld type registry.
pub fn encode_save_file(header: SaveHeader, body: &str) -> String {
    format!(
        "{SAVE_MAGIC}\n(format_version: {}, worldgen_seed: {})\n---\n{body}",
        header.format_version, header.worldgen_seed
    )
}

/// Classifies a save before its DynamicWorld body is deserialized.
///
/// Magic-less files are the only legacy v0 form accepted. A file that declares
/// a header must use the current version and a valid separator.
pub fn decode_save_file(contents: &str) -> Result<DecodedSaveFile<'_>, SaveFormatError> {
    let Some(after_magic) = contents.strip_prefix(SAVE_MAGIC) else {
        return Ok(DecodedSaveFile {
            format: SaveFormat::LegacyV0,
            body: contents,
        });
    };

    let header_and_body = after_magic
        .strip_prefix("\r\n")
        .or_else(|| after_magic.strip_prefix('\n'))
        .ok_or(SaveFormatError::MissingHeaderLineBreak)?;
    let (header_text, body) = header_and_body
        .split_once("\n---\n")
        .or_else(|| header_and_body.split_once("\r\n---\r\n"))
        .ok_or(SaveFormatError::MissingBodySeparator)?;
    let header = ron::from_str::<SaveHeader>(header_text)
        .map_err(|error| SaveFormatError::InvalidHeader(error.to_string()))?;

    if header.format_version != CURRENT_SAVE_FORMAT_VERSION {
        return Err(SaveFormatError::UnsupportedVersion {
            found: header.format_version,
            current: CURRENT_SAVE_FORMAT_VERSION,
        });
    }

    Ok(DecodedSaveFile {
        format: SaveFormat::V1(header),
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVALID_DYNAMIC_WORLD_BODY: &str = "this body is deliberately not DynamicWorld RON";

    #[test]
    fn v1_header_is_decoded_without_reading_the_body() {
        let encoded = encode_save_file(SaveHeader::current(42), INVALID_DYNAMIC_WORLD_BODY);

        let decoded = decode_save_file(&encoded).expect("v1 header should decode");

        assert_eq!(decoded.format, SaveFormat::V1(SaveHeader::current(42)));
        assert_eq!(decoded.body, INVALID_DYNAMIC_WORLD_BODY);
    }

    #[test]
    fn magic_less_file_is_classified_as_legacy_v0() {
        let decoded = decode_save_file(INVALID_DYNAMIC_WORLD_BODY).expect("legacy v0 is accepted");

        assert_eq!(decoded.format, SaveFormat::LegacyV0);
        assert_eq!(decoded.body, INVALID_DYNAMIC_WORLD_BODY);
    }

    #[test]
    fn future_header_version_is_rejected_before_body_deserialization() {
        let encoded = encode_save_file(
            SaveHeader {
                format_version: CURRENT_SAVE_FORMAT_VERSION + 1,
                worldgen_seed: 42,
            },
            INVALID_DYNAMIC_WORLD_BODY,
        );

        assert_eq!(
            decode_save_file(&encoded),
            Err(SaveFormatError::UnsupportedVersion {
                found: CURRENT_SAVE_FORMAT_VERSION + 1,
                current: CURRENT_SAVE_FORMAT_VERSION,
            })
        );
    }

    #[test]
    fn malformed_header_is_rejected_before_body_deserialization() {
        let contents = format!("{SAVE_MAGIC}\nnot valid RON\n---\n{INVALID_DYNAMIC_WORLD_BODY}");

        assert!(matches!(
            decode_save_file(&contents),
            Err(SaveFormatError::InvalidHeader(_))
        ));
    }
}
