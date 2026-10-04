//! Exclusive temporary snapshots shared by profiling-only observation producers.
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) fn temporary_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_owned();
    path.push(".tmp");
    PathBuf::from(path)
}

pub(super) fn require_fresh_path(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "observation path is already occupied",
        )),
    }
}

pub(super) fn require_fresh_output(output: &Path) -> std::io::Result<()> {
    require_fresh_path(output)?;
    require_fresh_path(&temporary_path(output))?;
    // Reject the old probe writer's temporary name as retained failure evidence.
    require_fresh_path(&output.with_extension("tmp"))
}

pub(super) fn write_observation(output: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = temporary_path(output);
    // A failed write remains evidence and blocks subsequent writes. Never follow
    // or truncate an existing temporary file, including a dangling symlink.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(PathBuf);

    impl Directory {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "hw-snapshot-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn both_probe_names_reject_retained_temporary_snapshots() {
        for name in ["session.bridge-trace.json", "session.m2-trace.json"] {
            let directory = Directory::new();
            let output = directory.0.join(name);
            require_fresh_output(&output).unwrap();
            write_observation(&output, b"first").unwrap();
            write_observation(&output, b"second").unwrap();
            std::fs::write(temporary_path(&output), b"retained failure").unwrap();
            assert!(require_fresh_output(&output).is_err());
            assert!(write_observation(&output, b"replacement").is_err());
            assert_eq!(std::fs::read(&output).unwrap(), b"second");
            assert_eq!(
                std::fs::read(temporary_path(&output)).unwrap(),
                b"retained failure"
            );
        }
    }

    #[test]
    fn legacy_temporary_and_failed_rename_remain_evidence() {
        let directory = Directory::new();
        let output = directory.0.join("session.bridge-trace.json");
        let legacy = output.with_extension("tmp");
        std::fs::write(&legacy, b"legacy failure").unwrap();
        assert!(require_fresh_output(&output).is_err());
        assert_eq!(std::fs::read(legacy).unwrap(), b"legacy failure");
        let blocked = directory.0.join("directory.json");
        std::fs::create_dir(&blocked).unwrap();
        assert!(write_observation(&blocked, b"failed rename").is_err());
        assert!(write_observation(&blocked, b"replacement").is_err());
        assert_eq!(
            std::fs::read(temporary_path(&blocked)).unwrap(),
            b"failed rename"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected_without_touching_their_targets() {
        let directory = Directory::new();
        let output = directory.0.join("session.m2-trace.json");
        let victim = directory.0.join("evidence");
        std::fs::write(&victim, b"original").unwrap();
        std::os::unix::fs::symlink(&victim, temporary_path(&output)).unwrap();
        assert!(require_fresh_output(&output).is_err());
        assert!(write_observation(&output, b"replacement").is_err());
        assert_eq!(std::fs::read(victim).unwrap(), b"original");
        let dangling = directory.0.join("dangling.json");
        std::os::unix::fs::symlink(directory.0.join("absent"), &dangling).unwrap();
        assert!(require_fresh_output(&dangling).is_err());
    }
}
