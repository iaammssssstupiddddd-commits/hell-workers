//! Platform atomic save commit: temp write, file sync, no-replace / replace, directory sync.

use std::fs::{self, File, OpenOptions};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use hw_core::SaveSlotId;

use super::format::{SaveHeader, write_container};

static NEXT_TEMP_SAVE_FILE_ID: AtomicU64 = AtomicU64::new(0);
const TEMP_FILE_ATTEMPTS: usize = 16;
const CRASH_TEMP_CLEANUP_LIMIT: usize = 32;
/// A concurrent process can have a live temp file. Only collect an old,
/// canonical-project temp name; never treat a generic `.tmp` as disposable.
const CRASH_TEMP_MINIMUM_AGE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicCommitMode {
    /// Fail if the target appears before commit. Never falls back to overwrite rename.
    NoReplace,
    /// Replace an existing target after the caller rechecked its revision.
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicCommitOutcome {
    Committed,
    /// File is visible at the target, but parent-directory durability is uncertain.
    CommittedDurabilityUncertain,
}

/// Measured boundaries of one atomic container write. These remain scalar
/// runtime diagnostics; only the perf scenario serializes them into an
/// artifact. `write_file_sync_ns` covers exclusive-temp creation, streamed
/// container write, and file sync. `commit_directory_sync_ns` covers the
/// no-replace/replace commit, temp cleanup, and parent-directory sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicSaveTimings {
    pub write_file_sync_ns: u64,
    pub commit_directory_sync_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicSaveResult {
    pub outcome: AtomicCommitOutcome,
    pub timings: AtomicSaveTimings,
}

#[derive(Debug)]
pub enum AtomicWriteError {
    Io(io::Error),
    /// No-replace commit found an existing target (or unsupported fallback).
    TargetExists,
}

impl std::fmt::Display for AtomicWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::TargetExists => formatter.write_str("save target already exists"),
        }
    }
}

impl std::error::Error for AtomicWriteError {}

/// Streams, syncs, commits, and records the two durability phase boundaries.
pub(crate) fn write_save_container_atomic_timed(
    target: &Path,
    header: SaveHeader,
    body: &str,
    mode: AtomicCommitMode,
) -> Result<AtomicSaveResult, AtomicWriteError> {
    let write_started = Instant::now();
    let (temporary_path, mut file) =
        create_temporary_save_file(target).map_err(AtomicWriteError::Io)?;
    let write_result = (|| -> io::Result<()> {
        write_container(header, body, &mut file)?;
        file.sync_all()
    })();
    drop(file);

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(AtomicWriteError::Io(error));
    }
    let write_file_sync_ns = elapsed_nanos(write_started.elapsed());

    let commit_started = Instant::now();
    let commit = match commit_temporary_file(&temporary_path, target, mode) {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
    };

    // Temp is either renamed away (Replace) or hard-linked then removed (NoReplace).
    if temporary_path.exists() {
        let _ = fs::remove_file(&temporary_path);
    }

    let outcome = match sync_parent_directory(target) {
        Ok(()) => commit,
        Err(_) => AtomicCommitOutcome::CommittedDurabilityUncertain,
    };
    Ok(AtomicSaveResult {
        outcome,
        timings: AtomicSaveTimings {
            write_file_sync_ns,
            commit_directory_sync_ns: elapsed_nanos(commit_started.elapsed()),
        },
    })
}

fn elapsed_nanos(duration: Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}

fn commit_temporary_file(
    temporary_path: &Path,
    target: &Path,
    mode: AtomicCommitMode,
) -> Result<AtomicCommitOutcome, AtomicWriteError> {
    match mode {
        AtomicCommitMode::Replace => {
            fs::rename(temporary_path, target).map_err(AtomicWriteError::Io)?;
            Ok(AtomicCommitOutcome::Committed)
        }
        AtomicCommitMode::NoReplace => commit_no_replace(temporary_path, target),
    }
}

fn commit_no_replace(
    temporary_path: &Path,
    target: &Path,
) -> Result<AtomicCommitOutcome, AtomicWriteError> {
    if target.exists() {
        return Err(AtomicWriteError::TargetExists);
    }

    match fs::hard_link(temporary_path, target) {
        Ok(()) => Ok(AtomicCommitOutcome::Committed),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(AtomicWriteError::TargetExists)
        }
        Err(error) if is_cross_device(&error) || is_unsupported_link(&error) => {
            // Never fall back to overwrite rename for Absent saves.
            Err(AtomicWriteError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("no-replace commit unsupported on this filesystem: {error}"),
            )))
        }
        Err(error) => Err(AtomicWriteError::Io(error)),
    }
}

fn is_cross_device(error: &io::Error) -> bool {
    error.raw_os_error() == Some(18) // EXDEV
}

fn is_unsupported_link(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
    ) || error.raw_os_error() == Some(95) // EOPNOTSUPP / EPERM variants
}

fn sync_parent_directory(target: &Path) -> io::Result<()> {
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = OpenOptions::new().read(true).open(parent)?;
    dir.sync_all()
}

fn create_temporary_save_file(path: &Path) -> io::Result<(PathBuf, File)> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    for _ in 0..TEMP_FILE_ATTEMPTS {
        let temporary_path = temporary_save_path(path);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique temporary save file",
    ))
}

pub(crate) fn temporary_save_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("world.scn.ron");
    let unique_id = NEXT_TEMP_SAVE_FILE_ID.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        unique_id
    ))
}

/// Removes a bounded number of crash temp files under `root` that match the
/// project temp prefix. Never deletes canonical slot filenames.
pub fn cleanup_crash_temp_files(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    let mut removed = 0usize;
    for entry in entries.flatten() {
        if removed >= CRASH_TEMP_CLEANUP_LIMIT {
            break;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_safe_crash_temp_file(&path, name) {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// True when `name` looks like a crash/temp save rather than a catalog slot.
pub fn is_crash_temp_file_name(name: &str) -> bool {
    let Some(stem) = name
        .strip_prefix('.')
        .and_then(|name| name.strip_suffix(".tmp"))
    else {
        return false;
    };
    let Some((with_process, unique_id)) = stem.rsplit_once('.') else {
        return false;
    };
    let Some((canonical_name, process_id)) = with_process.rsplit_once('.') else {
        return false;
    };
    process_id.parse::<u32>().is_ok()
        && unique_id.parse::<u64>().is_ok()
        && SaveSlotId::ALL
            .iter()
            .any(|slot| slot.canonical_file_name() == canonical_name)
}

fn is_safe_crash_temp_file(path: &Path, name: &str) -> bool {
    if !is_crash_temp_file_name(name) {
        return false;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    metadata
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age >= CRASH_TEMP_MINIMUM_AGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::format::SaveHeader;
    use std::fs::FileTimes;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir() -> PathBuf {
        let id = NEXT_TEMP_SAVE_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "hell-workers-atomic-{}-{}-{}",
            std::process::id(),
            id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn temporary_paths_are_unique_for_the_same_save_file() {
        let path = Path::new("saves/world.scn.ron");
        assert_ne!(temporary_save_path(path), temporary_save_path(path));
    }

    #[test]
    fn absent_no_replace_creates_file_and_rejects_second_commit() {
        let dir = unique_dir();
        let target = dir.join("manual-1.scn.ron");
        let body = "body-one";
        let first = write_save_container_atomic_timed(
            &target,
            SaveHeader::current(1),
            body,
            AtomicCommitMode::NoReplace,
        )
        .unwrap()
        .outcome;
        assert_eq!(first, AtomicCommitOutcome::Committed);
        assert!(target.is_file());
        assert!(
            write_save_container_atomic_timed(
                &target,
                SaveHeader::current(1),
                "body-two",
                AtomicCommitMode::NoReplace,
            )
            .is_err()
        );
        let contents = fs::read_to_string(&target).unwrap();
        assert!(contents.contains("body-one"));
        assert!(!contents.contains("body-two"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn replace_mode_overwrites_existing_target() {
        let dir = unique_dir();
        let target = dir.join("manual-2.scn.ron");
        write_save_container_atomic_timed(
            &target,
            SaveHeader::current(2),
            "old",
            AtomicCommitMode::NoReplace,
        )
        .unwrap();
        write_save_container_atomic_timed(
            &target,
            SaveHeader::current(2),
            "new",
            AtomicCommitMode::Replace,
        )
        .unwrap();
        let contents = fs::read_to_string(&target).unwrap();
        assert!(contents.contains("new"));
        assert!(!contents.contains("old"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn timed_write_reports_nonnegative_durability_phase_boundaries() {
        let dir = unique_dir();
        let target = dir.join("manual-3.scn.ron");
        let result = write_save_container_atomic_timed(
            &target,
            SaveHeader::current(3),
            "timed-body",
            AtomicCommitMode::NoReplace,
        )
        .unwrap();
        assert_eq!(result.outcome, AtomicCommitOutcome::Committed);
        assert!(target.is_file());
        // The exact duration is platform-dependent, but both phases are
        // always represented as bounded integer metrics.
        let _ = result.timings.write_file_sync_ns;
        let _ = result.timings.commit_directory_sync_ns;
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn crash_temps_are_ignored_by_name_helper_and_cleaned() {
        let dir = unique_dir();
        let temp = dir.join(".manual-1.scn.ron.1.2.tmp");
        fs::write(&temp, b"junk").unwrap();
        File::open(&temp)
            .unwrap()
            .set_times(FileTimes::new().set_modified(SystemTime::now() - CRASH_TEMP_MINIMUM_AGE))
            .unwrap();
        fs::write(dir.join("manual-1.scn.ron"), b"keep").unwrap();
        assert!(is_crash_temp_file_name(
            temp.file_name().unwrap().to_str().unwrap()
        ));
        assert_eq!(cleanup_crash_temp_files(&dir), 1);
        assert!(!temp.exists());
        assert!(dir.join("manual-1.scn.ron").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn crash_temp_helper_rejects_generic_and_noncanonical_tmp_names() {
        assert!(!is_crash_temp_file_name(".manual-9.scn.ron.1.2.tmp"));
        assert!(!is_crash_temp_file_name(".manual-1.scn.ron.backup.tmp"));
        assert!(!is_crash_temp_file_name(".notes.scn.ron.1.2.tmp"));
        assert!(!is_crash_temp_file_name("manual-1.scn.ron.1.2.tmp"));
    }
}
