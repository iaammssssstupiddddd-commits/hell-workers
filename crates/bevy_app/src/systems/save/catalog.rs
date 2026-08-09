//! Runtime save catalog: bounded header scan without DynamicWorld deserialize.

use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use hw_core::{GameSettings, SaveSlotId, SaveSlotRole};

use crate::world::map::GeneratedWorldLayoutResource;

use super::format::{
    SAVE_HEADER_INSPECT_LIMIT_BYTES, SaveHeaderStatus, inspect_save_prefix,
    save_header_status_allows_load,
};

/// Default relative storage root used by production launches.
pub const DEFAULT_SAVE_STORAGE_ROOT: &str = "saves";

/// Root directory that owns every slot file. Tests inject a unique temp path.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct SaveStorageRoot(pub PathBuf);

impl SaveStorageRoot {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn resolve(&self, slot: SaveSlotId) -> PathBuf {
        self.0.join(slot.canonical_file_name())
    }
}

impl Default for SaveStorageRoot {
    fn default() -> Self {
        Self::new(DEFAULT_SAVE_STORAGE_ROOT)
    }
}

/// Authoritative identity of an existing (or absent) slot file at request time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SaveFileRevision {
    pub exists: bool,
    pub length: u64,
    pub modified: Option<SystemTime>,
    pub file_identity: Option<u64>,
    pub prefix_fingerprint: u64,
}

impl SaveFileRevision {
    pub const fn absent() -> Self {
        Self {
            exists: false,
            length: 0,
            modified: None,
            file_identity: None,
            prefix_fingerprint: 0,
        }
    }
}

/// Expected target bound into a save request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SaveExpectedTarget {
    Absent,
    Exact(SaveFileRevision),
}

/// Orthogonal write/load capabilities for a slot role + generation setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SaveSlotCapabilities {
    pub can_manual_save: bool,
    pub can_scheduler_save: bool,
    pub can_load: bool,
}

/// Content health. Kept separate from role/capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveContentStatus {
    Empty,
    CurrentV1 {
        worldgen_seed: u64,
    },
    LegacyV0Candidate,
    SeedMismatch {
        worldgen_seed: u64,
    },
    UnsupportedVersion {
        found: u32,
    },
    CorruptHeader,
    Unreadable,
    /// Set only after a failed full load attempt; file bytes may still exist.
    BodyInvalid,
}

impl SaveContentStatus {
    pub fn from_header(status: SaveHeaderStatus) -> Self {
        match status {
            SaveHeaderStatus::Empty => Self::Empty,
            SaveHeaderStatus::CurrentV1 { worldgen_seed } => Self::CurrentV1 { worldgen_seed },
            SaveHeaderStatus::LegacyV0Candidate => Self::LegacyV0Candidate,
            SaveHeaderStatus::SeedMismatch { worldgen_seed } => {
                Self::SeedMismatch { worldgen_seed }
            }
            SaveHeaderStatus::UnsupportedVersion { found } => Self::UnsupportedVersion { found },
            SaveHeaderStatus::CorruptHeader => Self::CorruptHeader,
            SaveHeaderStatus::Unreadable => Self::Unreadable,
        }
    }

    pub const fn allows_load(self) -> bool {
        matches!(
            self,
            Self::CurrentV1 { .. } | Self::LegacyV0Candidate | Self::BodyInvalid
        )
    }

    pub fn player_label(self) -> &'static str {
        match self {
            Self::Empty => {
                super::format::save_header_status_label(super::format::SaveHeaderStatus::Empty)
            }
            Self::CurrentV1 { .. } => super::format::save_header_status_label(
                super::format::SaveHeaderStatus::CurrentV1 { worldgen_seed: 0 },
            ),
            Self::LegacyV0Candidate => super::format::save_header_status_label(
                super::format::SaveHeaderStatus::LegacyV0Candidate,
            ),
            Self::SeedMismatch { .. } => super::format::save_header_status_label(
                super::format::SaveHeaderStatus::SeedMismatch { worldgen_seed: 0 },
            ),
            Self::UnsupportedVersion { found } => super::format::save_header_status_label(
                super::format::SaveHeaderStatus::UnsupportedVersion { found },
            ),
            Self::CorruptHeader => super::format::save_header_status_label(
                super::format::SaveHeaderStatus::CorruptHeader,
            ),
            Self::Unreadable => {
                super::format::save_header_status_label(super::format::SaveHeaderStatus::Unreadable)
            }
            Self::BodyInvalid => "Invalid save data",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveCatalogEntry {
    pub slot: SaveSlotId,
    pub role: SaveSlotRole,
    pub capabilities: SaveSlotCapabilities,
    pub content: SaveContentStatus,
    pub revision: SaveFileRevision,
    pub file_size: u64,
    pub modified: Option<SystemTime>,
    /// True when metadata lookup failed while the file itself was readable.
    pub modified_unavailable: bool,
}

impl SaveCatalogEntry {
    pub fn player_label(&self) -> &'static str {
        self.slot.player_label()
    }

    pub fn content_label(&self) -> &'static str {
        self.content.player_label()
    }
}

/// Runtime-only index. Never written into DynamicWorld.
#[derive(Resource, Debug, Clone, Default)]
pub struct SaveCatalog {
    entries: Vec<SaveCatalogEntry>,
    dirty: bool,
    last_scan_bytes: usize,
    last_scan_entry_count: usize,
    body_invalid_slots: Vec<SaveSlotId>,
    scanned_generation_limit: Option<u8>,
}

impl SaveCatalog {
    pub fn entries(&self) -> &[SaveCatalogEntry] {
        &self.entries
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Records a failed full-body validation without putting runtime state in
    /// the save file or world snapshot.
    pub fn mark_body_invalid(&mut self, slot: SaveSlotId) {
        if !self.body_invalid_slots.contains(&slot) {
            self.body_invalid_slots.push(slot);
        }
        self.mark_dirty();
    }

    pub fn clear_body_invalid(&mut self, slot: SaveSlotId) {
        self.body_invalid_slots
            .retain(|candidate| *candidate != slot);
    }

    pub fn body_invalid_slots(&self) -> &[SaveSlotId] {
        &self.body_invalid_slots
    }

    pub fn needs_refresh(&self, generations: AutosaveGenerationLimit) -> bool {
        self.dirty
            || self
                .scanned_generation_limit
                .is_some_and(|scanned| scanned != generations.clamped())
    }

    pub fn last_scan_bytes(&self) -> usize {
        self.last_scan_bytes
    }

    pub fn last_scan_entry_count(&self) -> usize {
        self.last_scan_entry_count
    }

    pub fn entry(&self, slot: SaveSlotId) -> Option<&SaveCatalogEntry> {
        self.entries.iter().find(|entry| entry.slot == slot)
    }

    pub fn replace_entries(&mut self, entries: Vec<SaveCatalogEntry>, bytes_read: usize) {
        self.last_scan_entry_count = entries.len();
        self.last_scan_bytes = bytes_read;
        self.body_invalid_slots.retain(|slot| {
            entries.iter().any(|entry| {
                entry.slot == *slot && entry.revision.exists && entry.content.allows_load()
            })
        });
        self.entries = entries;
        self.dirty = false;
    }
}

/// Active autosave generation count from settings (`1..=5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutosaveGenerationLimit(pub u8);

impl Default for AutosaveGenerationLimit {
    fn default() -> Self {
        Self(3)
    }
}

impl AutosaveGenerationLimit {
    pub fn clamped(self) -> u8 {
        self.0.clamp(1, 5)
    }

    pub fn is_active(self, slot: SaveSlotId) -> bool {
        slot.autosave_generation()
            .is_some_and(|generation| generation <= self.clamped())
    }
}

pub fn slot_capabilities(
    slot: SaveSlotId,
    content: SaveContentStatus,
    generations: AutosaveGenerationLimit,
) -> SaveSlotCapabilities {
    let can_load = content.allows_load();
    match slot.role() {
        SaveSlotRole::Manual => SaveSlotCapabilities {
            can_manual_save: true,
            can_scheduler_save: false,
            can_load,
        },
        SaveSlotRole::Autosave => SaveSlotCapabilities {
            can_manual_save: false,
            can_scheduler_save: generations.is_active(slot),
            can_load,
        },
        SaveSlotRole::LegacyDefault => SaveSlotCapabilities {
            can_manual_save: false,
            can_scheduler_save: false,
            can_load,
        },
    }
}

/// Relative age label template from a cached timestamp. Does not touch the filesystem.
pub fn relative_modified_label(modified: Option<SystemTime>, now: SystemTime) -> &'static str {
    let Some(modified) = modified else {
        return "Modified time unavailable";
    };
    let Ok(age) = now.duration_since(modified) else {
        return "just now";
    };
    if age < Duration::from_secs(60) {
        "just now"
    } else if age < Duration::from_secs(60 * 60) {
        "Xm ago"
    } else if age < Duration::from_secs(60 * 60 * 24) {
        "Xh ago"
    } else {
        "Xd ago"
    }
}

/// Formats a concrete relative age for UI presentation.
pub fn format_relative_modified(modified: Option<SystemTime>, now: SystemTime) -> String {
    let Some(modified) = modified else {
        return "Modified time unavailable".to_owned();
    };
    let Ok(age) = now.duration_since(modified) else {
        return "just now".to_owned();
    };
    if age < Duration::from_secs(60) {
        "just now".to_owned()
    } else if age < Duration::from_secs(60 * 60) {
        format!("{}m ago", age.as_secs() / 60)
    } else if age < Duration::from_secs(60 * 60 * 24) {
        format!("{}h ago", age.as_secs() / 3600)
    } else {
        format!("{}d ago", age.as_secs() / 86_400)
    }
}

#[derive(Debug, Clone)]
pub struct CatalogScanCounters {
    pub bytes_read: usize,
    pub entries: usize,
}

/// Scans fixed slot IDs under `root`. Legacy default is included only when present.
pub fn scan_save_catalog(
    root: &SaveStorageRoot,
    expected_worldgen_seed: Option<u64>,
    generations: AutosaveGenerationLimit,
    previous_body_invalid: &[SaveSlotId],
) -> (Vec<SaveCatalogEntry>, CatalogScanCounters) {
    let mut entries = Vec::with_capacity(9);
    let mut bytes_read = 0usize;

    for slot in SaveSlotId::MANUAL.into_iter().chain(SaveSlotId::AUTOSAVE) {
        let (entry, read) = inspect_slot(
            root,
            slot,
            expected_worldgen_seed,
            generations,
            previous_body_invalid,
        );
        bytes_read = bytes_read.saturating_add(read);
        entries.push(entry);
    }

    let legacy_path = root.resolve(SaveSlotId::LegacyDefault);
    if legacy_path.exists() {
        let (entry, read) = inspect_slot(
            root,
            SaveSlotId::LegacyDefault,
            expected_worldgen_seed,
            generations,
            previous_body_invalid,
        );
        bytes_read = bytes_read.saturating_add(read);
        entries.push(entry);
    }

    let counters = CatalogScanCounters {
        bytes_read,
        entries: entries.len(),
    };
    (entries, counters)
}

pub fn refresh_save_catalog(
    catalog: &mut SaveCatalog,
    root: &SaveStorageRoot,
    expected_worldgen_seed: Option<u64>,
    generations: AutosaveGenerationLimit,
) {
    let _ = super::atomic_file::cleanup_crash_temp_files(root.as_path());
    let previous_body_invalid = catalog.body_invalid_slots().to_vec();
    let (entries, counters) = scan_save_catalog(
        root,
        expected_worldgen_seed,
        generations,
        &previous_body_invalid,
    );
    catalog.replace_entries(entries, counters.bytes_read);
    catalog.scanned_generation_limit = Some(generations.clamped());
}

/// Re-scans only after a terminal operation or a generation-count change. It
/// runs before presentation and autosave selection so a successful autosave is
/// visible to—and considered by—the next scheduler decision.
pub(crate) fn refresh_dirty_save_catalog_system(
    mut catalog: ResMut<SaveCatalog>,
    root: Res<SaveStorageRoot>,
    settings: Res<GameSettings>,
    worldgen_layout: Option<Res<GeneratedWorldLayoutResource>>,
) {
    let generations = AutosaveGenerationLimit(settings.normalized_autosave_generations());
    if !catalog.needs_refresh(generations) {
        return;
    }
    refresh_save_catalog(
        &mut catalog,
        &root,
        worldgen_layout.as_ref().map(|layout| layout.master_seed),
        generations,
    );
}

/// Reads an authoritative revision for request binding / Last recheck.
pub fn read_save_file_revision(path: &Path) -> Result<SaveFileRevision, std::io::Error> {
    let (revision, _, _, _) = inspect_save_file(path, None)?;
    Ok(revision)
}

/// Inspects one path with an expected live seed for catalog classification.
pub fn inspect_save_file(
    path: &Path,
    expected_worldgen_seed: Option<u64>,
) -> Result<(SaveFileRevision, SaveHeaderStatus, usize, bool), std::io::Error> {
    if !path.exists() {
        return Ok((
            SaveFileRevision::absent(),
            SaveHeaderStatus::Empty,
            0,
            false,
        ));
    }
    if path.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::IsADirectory,
            "save slot path is a directory",
        ));
    }
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 {
        return Ok((
            SaveFileRevision {
                exists: true,
                length: 0,
                modified: metadata.modified().ok(),
                file_identity: file_identity_from_metadata(&metadata),
                prefix_fingerprint: 0,
            },
            SaveHeaderStatus::Empty,
            0,
            metadata.modified().is_err(),
        ));
    }
    let mut file = File::open(path)?;
    let mut buffer = vec![0_u8; SAVE_HEADER_INSPECT_LIMIT_BYTES];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);
    let prefix = String::from_utf8_lossy(&buffer);
    let inspected = inspect_save_prefix(&prefix, bytes_read, expected_worldgen_seed);
    Ok((
        SaveFileRevision {
            exists: true,
            length: metadata.len(),
            modified: metadata.modified().ok(),
            file_identity: file_identity_from_metadata(&metadata),
            prefix_fingerprint: fingerprint_bytes(&buffer),
        },
        inspected.status,
        bytes_read,
        metadata.modified().is_err(),
    ))
}

fn inspect_slot(
    root: &SaveStorageRoot,
    slot: SaveSlotId,
    expected_worldgen_seed: Option<u64>,
    generations: AutosaveGenerationLimit,
    previous_body_invalid: &[SaveSlotId],
) -> (SaveCatalogEntry, usize) {
    let path = root.resolve(slot);
    match inspect_save_file(&path, expected_worldgen_seed) {
        Ok((revision, header_status, bytes_read, modified_unavailable)) => {
            if !revision.exists {
                let content = SaveContentStatus::Empty;
                return (
                    SaveCatalogEntry {
                        slot,
                        role: slot.role(),
                        capabilities: slot_capabilities(slot, content, generations),
                        content,
                        revision,
                        file_size: 0,
                        modified: None,
                        modified_unavailable: false,
                    },
                    0,
                );
            }
            let mut content = SaveContentStatus::from_header(header_status);
            if previous_body_invalid.contains(&slot) && content.allows_load() {
                content = SaveContentStatus::BodyInvalid;
            }
            let mut capabilities = slot_capabilities(slot, content, generations);
            capabilities.can_load = content.allows_load();
            let _ = save_header_status_allows_load(header_status);
            (
                SaveCatalogEntry {
                    slot,
                    role: slot.role(),
                    capabilities,
                    content,
                    revision: revision.clone(),
                    file_size: revision.length,
                    modified: revision.modified,
                    modified_unavailable,
                },
                bytes_read,
            )
        }
        Err(_) => {
            let content = SaveContentStatus::Unreadable;
            (
                SaveCatalogEntry {
                    slot,
                    role: slot.role(),
                    capabilities: slot_capabilities(slot, content, generations),
                    content,
                    revision: SaveFileRevision {
                        exists: true,
                        length: 0,
                        modified: None,
                        file_identity: None,
                        prefix_fingerprint: 0,
                    },
                    file_size: 0,
                    modified: None,
                    modified_unavailable: true,
                },
                0,
            )
        }
    }
}

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn file_identity_from_metadata(metadata: &fs::Metadata) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(metadata.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::format::{SaveHeader, encode_save_file};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_root() -> SaveStorageRoot {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "hell-workers-catalog-{}-{}-{}",
            std::process::id(),
            id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        SaveStorageRoot::new(path)
    }

    #[test]
    fn scan_limits_entries_and_header_bytes() {
        let root = unique_root();
        fs::write(
            root.resolve(SaveSlotId::Manual1),
            encode_save_file(SaveHeader::current(1), &"x".repeat(64_000)),
        )
        .unwrap();
        fs::write(root.resolve(SaveSlotId::LegacyDefault), b"legacy-ish").unwrap();

        let (entries, counters) =
            scan_save_catalog(&root, Some(1), AutosaveGenerationLimit(3), &[]);

        assert!(counters.entries <= 9);
        assert!(counters.bytes_read <= SAVE_HEADER_INSPECT_LIMIT_BYTES * counters.entries);
        assert!(
            entries
                .iter()
                .any(|entry| entry.slot == SaveSlotId::LegacyDefault)
        );
        let manual = entries
            .iter()
            .find(|entry| entry.slot == SaveSlotId::Manual1)
            .unwrap();
        assert!(matches!(
            manual.content,
            SaveContentStatus::CurrentV1 { worldgen_seed: 1 }
        ));
        assert!(manual.capabilities.can_manual_save);
        assert!(!manual.capabilities.can_scheduler_save);
        assert!(manual.capabilities.can_load);
        let _ = fs::remove_dir_all(root.as_path());
    }

    #[test]
    fn missing_legacy_default_is_omitted_not_padded() {
        let root = unique_root();
        let (entries, _) = scan_save_catalog(&root, Some(1), AutosaveGenerationLimit(3), &[]);
        assert_eq!(entries.len(), 8);
        assert!(
            !entries
                .iter()
                .any(|entry| entry.slot == SaveSlotId::LegacyDefault)
        );
        let _ = fs::remove_dir_all(root.as_path());
    }

    #[test]
    fn magicless_file_is_legacy_candidate_and_capabilities_stay_orthogonal() {
        let root = unique_root();
        fs::write(root.resolve(SaveSlotId::Manual2), b"no-magic-body").unwrap();
        fs::write(
            root.resolve(SaveSlotId::Autosave4),
            encode_save_file(SaveHeader::current(9), "body"),
        )
        .unwrap();

        let (entries, _) = scan_save_catalog(&root, Some(1), AutosaveGenerationLimit(3), &[]);
        let manual = entries
            .iter()
            .find(|entry| entry.slot == SaveSlotId::Manual2)
            .unwrap();
        assert_eq!(manual.content, SaveContentStatus::LegacyV0Candidate);
        assert!(manual.capabilities.can_manual_save);
        assert!(manual.capabilities.can_load);

        let inactive = entries
            .iter()
            .find(|entry| entry.slot == SaveSlotId::Autosave4)
            .unwrap();
        assert!(!inactive.capabilities.can_scheduler_save);
        assert!(matches!(
            inactive.content,
            SaveContentStatus::SeedMismatch { worldgen_seed: 9 }
        ));
        assert!(!inactive.content.allows_load());
        assert!(!inactive.capabilities.can_load);
        let _ = fs::remove_dir_all(root.as_path());
    }

    #[test]
    fn relative_mtime_labels_do_not_require_rescan() {
        let now = SystemTime::now();
        assert_eq!(
            relative_modified_label(None, now),
            "Modified time unavailable"
        );
        assert_eq!(
            relative_modified_label(Some(now - Duration::from_secs(5)), now),
            "just now"
        );
        assert_eq!(
            format_relative_modified(Some(now - Duration::from_secs(120)), now),
            "2m ago"
        );
    }

    #[test]
    fn unreadable_path_reports_player_safe_status() {
        let root = unique_root();
        let path = root.resolve(SaveSlotId::Manual3);
        fs::write(&path, b"secret").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        // Remove read permission where supported so open/read fails.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o000);
            fs::set_permissions(&path, permissions).unwrap();
        }
        #[cfg(not(unix))]
        {
            fs::set_permissions(&path, permissions).unwrap();
            // Fallback: replace with a directory so open/read cannot succeed as a save.
            fs::remove_file(&path).unwrap();
            fs::create_dir_all(&path).unwrap();
        }

        let (entries, _) = scan_save_catalog(&root, Some(1), AutosaveGenerationLimit(3), &[]);
        let entry = entries
            .iter()
            .find(|entry| entry.slot == SaveSlotId::Manual3)
            .unwrap();
        assert_eq!(
            entry.content,
            SaveContentStatus::Unreadable,
            "entry={entry:?}"
        );
        assert_eq!(entry.content_label(), "Save file unavailable");
        assert!(!entry.capabilities.can_load);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o600);
            fs::set_permissions(&path, permissions).unwrap();
        }
        let _ = fs::remove_dir_all(root.as_path());
    }
}
