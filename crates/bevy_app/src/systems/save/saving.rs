//! ワールドのセーブ（exclusive system）。
//!
//! `schema.rs` が構成する DynamicWorld allow-listで構築し、RON にシリアライズして
//! ファイルへ書き込む。
//!
//! # 設計上の逸脱（plan からの変更点）
//! plan の Phase A は「セーブ前にライブワールドを正規化する」（例: `AssignedTask` を
//! `None` にリセットする）ことを想定していたが、本実装ではその代わりに
//! **allow-list に含めない**（`AssignedTask` 等のタスク実行中状態を deny）方式を
//! 採用した。ライブゲームの状態を一切変更せずに済み、`unassign_task` の呼び出しに
//! 伴う予約解放処理を経由する必要もない。詳細は `docs/save_load.md` を参照。

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use bevy::prelude::*;

use super::format::{SaveHeader, encode_save_file};
use super::schema::{build_persisted_world, collect_persisted_entities};
use super::state::{SaveLoadFailureKind, SaveLoadResult, SavePath};

static NEXT_TEMP_SAVE_FILE_ID: AtomicU64 = AtomicU64::new(0);
const TEMP_FILE_ATTEMPTS: usize = 16;

#[derive(Debug)]
enum SaveExecutionError {
    Serialize(String),
    Write(io::Error),
}

impl SaveExecutionError {
    const fn failure_kind(&self) -> SaveLoadFailureKind {
        match self {
            Self::Serialize(_) => SaveLoadFailureKind::SaveSerialize,
            Self::Write(_) => SaveLoadFailureKind::SaveWrite,
        }
    }
}

impl fmt::Display for SaveExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(error) => {
                write!(formatter, "DynamicWorld serialization failed: {error}")
            }
            Self::Write(error) => write!(formatter, "save file write failed: {error}"),
        }
    }
}

pub(super) fn save_world_system(world: &mut World) -> SaveLoadResult {
    let started = Instant::now();
    let master_seed = world
        .resource::<crate::world::map::GeneratedWorldLayoutResource>()
        .master_seed;
    let save_path = world.resource::<SavePath>().as_path().to_path_buf();

    let execution = execute_save_with(
        || {
            let body = serialize_world_body(world)?;
            Ok(encode_save_file(SaveHeader::current(master_seed), &body))
        },
        |contents| write_save_file(&save_path, contents),
    );

    if let Err(error) = execution {
        error!("Failed to save world to {}: {error}", save_path.display());
        return SaveLoadResult::Failed(error.failure_kind());
    }

    let elapsed = started.elapsed();
    if elapsed.as_millis() > 100 {
        warn!("Save took {elapsed:?} (>100ms)");
    } else {
        info!("World saved to {} in {elapsed:?}", save_path.display());
    }
    SaveLoadResult::Succeeded
}

/// Runs the production encode/write branch behind a small injectable seam so
/// both failure paths use the same classification in tests and at runtime.
fn execute_save_with(
    encode: impl FnOnce() -> Result<String, SaveExecutionError>,
    write: impl FnOnce(&str) -> io::Result<()>,
) -> Result<(), SaveExecutionError> {
    let contents = encode()?;
    write(&contents).map_err(SaveExecutionError::Write)
}

/// Serializes the persisted simulation state without doing filesystem I/O.
fn serialize_world_body(world: &mut World) -> Result<String, SaveExecutionError> {
    let type_registry = world
        .resource::<bevy::ecs::reflect::AppTypeRegistry>()
        .clone();
    let registry = type_registry.read();

    let target_entities = collect_persisted_entities(world);

    let dynamic_world = build_persisted_world(world, &registry, target_entities.into_iter());

    dynamic_world
        .serialize(&registry)
        .map_err(|error| SaveExecutionError::Serialize(error.to_string()))
}

/// 同じディレクトリ内の一意な一時ファイルへ書き込んでから rename する。
/// 途中でクラッシュしても既存のセーブファイルは破損しない。
fn write_save_file(path: &Path, contents: &str) -> io::Result<()> {
    let (temporary_path, mut file) = create_temporary_save_file(path)?;
    let write_result = (|| -> io::Result<()> {
        file.write_all(contents.as_bytes())?;
        file.sync_all()
    })();
    drop(file);

    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(error);
    }

    if let Err(error) = std::fs::rename(&temporary_path, path) {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(error);
    }

    Ok(())
}

fn create_temporary_save_file(path: &Path) -> io::Result<(PathBuf, File)> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

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

fn temporary_save_path(path: &Path) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(result: Result<(), SaveExecutionError>) -> SaveLoadResult {
        match result {
            Ok(()) => SaveLoadResult::Succeeded,
            Err(error) => SaveLoadResult::Failed(error.failure_kind()),
        }
    }

    fn unique_test_directory() -> PathBuf {
        let unique_id = NEXT_TEMP_SAVE_FILE_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "hell-workers-save-test-{}.{}",
            std::process::id(),
            unique_id
        ))
    }

    #[test]
    fn temporary_paths_are_unique_for_the_same_save_file() {
        let path = Path::new("saves/world.scn.ron");

        assert_ne!(temporary_save_path(path), temporary_save_path(path));
    }

    #[test]
    fn injectable_execution_classifies_success_and_both_failure_stages() {
        assert_eq!(
            outcome(execute_save_with(|| Ok("encoded".to_owned()), |_| Ok(()))),
            SaveLoadResult::Succeeded
        );
        assert_eq!(
            outcome(execute_save_with(
                || Err(SaveExecutionError::Serialize("details".to_owned())),
                |_| panic!("write must not run after encode failure")
            )),
            SaveLoadResult::Failed(SaveLoadFailureKind::SaveSerialize)
        );
        assert_eq!(
            outcome(execute_save_with(
                || Ok("encoded".to_owned()),
                |_| Err(io::Error::other("details"))
            )),
            SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite)
        );
    }

    #[test]
    fn atomic_write_replaces_the_target_without_leaving_temp_files() {
        let directory = unique_test_directory();
        let path = directory.join("world.scn.ron");

        write_save_file(&path, "first save").expect("first write should succeed");
        write_save_file(&path, "second save").expect("replacement write should succeed");

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second save");
        let remaining_temp_files = std::fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(remaining_temp_files, 0);

        std::fs::remove_dir_all(directory).expect("test directory should be removable");
    }
}
