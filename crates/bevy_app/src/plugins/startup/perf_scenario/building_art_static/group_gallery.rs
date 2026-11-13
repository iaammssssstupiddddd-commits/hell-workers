//! Owned-window group-art capture. There is no performance or release verdict.

mod projection;
mod release_context;
pub(super) use release_context::FloorInventory as ReleasedFloorInventory;

use std::{
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;
use crate::assets::building_asset_set::gallery_capture::GalleryCaptureSession;
use crate::assets::building_asset_set::group_art::{GROUP_ART_HEIGHT, GROUP_ART_WIDTH};
use serde::{Deserialize, Serialize};

#[derive(Default, Resource)]
struct CaptureState {
    ui_prepared: bool,
    started_at: Option<std::time::Instant>,
    ready: Option<Value>,
    ready_sha256: String,
    completed: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct CaptureAck {
    schema_version: u8,
    nonce: String,
    pid: u32,
    probe_sha256: String,
    png_sha256: String,
    window_id: String,
    width: u32,
    height: u32,
    captured_at_unix_ms: u64,
}

fn now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "gallery clock overflow".into())
}

fn sha(bytes: &[u8]) -> String {
    hw_infra::lighting::digest_hex(&Sha256::digest(bytes).into())
}

fn write_fresh(path: &Path, value: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("partial");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    // Link publication is atomic and cannot replace a foreign/stale destination.
    std::fs::hard_link(&temporary, path).map_err(|e| e.to_string())?;
    std::fs::remove_file(temporary).map_err(|e| e.to_string())?;
    Ok(sha(&bytes))
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn validate_ack(
    ack: &CaptureAck,
    session: &GalleryCaptureSession,
    ready: &Value,
    digest: &str,
    now: u64,
) -> Result<(), String> {
    if ack.schema_version != 1
        || ack.nonce != session.nonce()
        || ack.pid != std::process::id()
        || ack.probe_sha256 != digest
        || !valid_hash(&ack.png_sha256)
        || !ack.window_id.starts_with("0x")
        || ack.window_id.len() < 3
        || ack.window_id.len() > 18
        || u64::from_str_radix(&ack.window_id[2..], 16)
            .ok()
            .is_none_or(|id| id == 0)
        || ack.width != GROUP_ART_WIDTH
        || ack.height != GROUP_ART_HEIGHT
        || ack.captured_at_unix_ms
            < ready["ready_at_unix_ms"]
                .as_u64()
                .ok_or("missing ready clock")?
        || ack.captured_at_unix_ms > now
        || now - ack.captured_at_unix_ms > 30_000
    {
        return Err("stale, foreign or incomplete group capture ACK".into());
    }
    Ok(())
}

pub(crate) fn observe(world: &mut World) {
    let Some(session) = world.get_resource::<GalleryCaptureSession>().cloned() else {
        return;
    };
    world.init_resource::<CaptureState>();
    let started = *world
        .resource_mut::<CaptureState>()
        .started_at
        .get_or_insert_with(std::time::Instant::now);
    let result = if started.elapsed().as_secs() >= 180 {
        Err("group gallery readiness/capture ACK deadline exceeded".into())
    } else {
        observe_inner(world, &session)
    };
    if let Err(reason) = result {
        error!("GROUP_ART_GALLERY: {reason}");
        world.resource_mut::<BuildingArtStaticState>().phase = Phase::Failed;
        world.write_message(AppExit::error());
    }
}

fn observe_inner(world: &mut World, session: &GalleryCaptureSession) -> Result<(), String> {
    let directory = session
        .status_path()
        .parent()
        .ok_or("missing gallery directory")?;
    let ack_path = directory.join("capture-ack.json");
    let final_path = directory.join("final.json");
    if world.resource::<CaptureState>().completed {
        return Ok(());
    }
    if !world.resource::<CaptureState>().ui_prepared {
        if ack_path.symlink_metadata().is_ok() || final_path.symlink_metadata().is_ok() {
            return Err("group capture ACK/final appeared before readiness".into());
        }
        if world.resource::<BuildingArtStaticState>().phase != Phase::Ready {
            return Ok(());
        }
        let roots = world
            .query_filtered::<Entity, With<Node>>()
            .iter(world)
            .collect::<Vec<_>>();
        for root in roots {
            world.entity_mut(root).insert(Visibility::Hidden);
        }
        world.resource_mut::<CaptureState>().ui_prepared = true;
        return Ok(());
    }
    if matches!(session, GalleryCaptureSession::Released(_))
        && world.resource::<CaptureState>().ready.is_none()
        && !release_context::prepare(world)?
    {
        if ack_path.symlink_metadata().is_ok()
            || final_path.symlink_metadata().is_ok()
            || session.status_path().symlink_metadata().is_ok()
        {
            return Err(
                "released capture artifact appeared during explicit context preparation".into(),
            );
        }
        return Ok(());
    }
    let state = world.resource::<BuildingArtStaticState>();
    if state.phase == Phase::Failed {
        return Err("group fixture failed".into());
    }
    if state.phase != Phase::Ready || state.stable_frames < 60 {
        if ack_path.symlink_metadata().is_ok() {
            return Err("premature group capture ACK".into());
        }
        return Ok(());
    }
    let frames = state.stable_frames;
    let fixture = state
        .m6_final_evidence
        .clone()
        .ok_or("missing actual group frame")?;
    let door = world
        .resource::<ProductionDoorAssetPool>()
        .resolved
        .as_ref()
        .ok_or("gallery requires resolved Door assets")?;
    session.check_door(&door.identity)?;
    let mut snapshot = projection::collect(world, session)?;
    if matches!(session, GalleryCaptureSession::Released(_)) {
        snapshot["context"] = release_context::snapshot(world)?;
    }
    if world.resource::<CaptureState>().ready.is_none() {
        let ready = json!({"schema_version": 1, "profile": session.contract(),
            "status": "ready", "nonce": session.nonce(), "pid": std::process::id(),
            "session": session, "stable_frames": frames, "ready_at_unix_ms": now_ms()?,
            "fixture": fixture, "snapshot": snapshot, "performance_evidence": false,
            "accepted": false, "promotion_authority": false});
        let digest = write_fresh(session.status_path(), &ready)?;
        let mut capture = world.resource_mut::<CaptureState>();
        capture.ready = Some(ready);
        capture.ready_sha256 = digest;
        return Ok(());
    }
    let capture = world.resource::<CaptureState>();
    let ready = capture.ready.clone().ok_or("missing ready evidence")?;
    let digest = capture.ready_sha256.clone();
    if ready["fixture"] != fixture || ready["snapshot"] != snapshot {
        return Err("group art state/camera/window changed after readiness".into());
    }
    let metadata = match ack_path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 8192 {
        return Err("group ACK must be a bounded regular file".into());
    }
    let bytes = std::fs::read(&ack_path).map_err(|e| e.to_string())?;
    let ack: CaptureAck = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate_ack(&ack, session, &ready, &digest, now_ms()?)?;
    if sha(&std::fs::read(session.status_path()).map_err(|e| e.to_string())?) != digest {
        return Err("group ready probe changed before ACK".into());
    }
    let png_path = directory.join("actual-window.png");
    let image_metadata = png_path.symlink_metadata().map_err(|e| e.to_string())?;
    if !image_metadata.is_file()
        || image_metadata.file_type().is_symlink()
        || image_metadata.len()
            > u64::from(GROUP_ART_WIDTH) * u64::from(GROUP_ART_HEIGHT) * 5 + 65536
    {
        return Err("group captured PNG is not a bounded regular file".into());
    }
    let png = std::fs::read(png_path).map_err(|e| e.to_string())?;
    if sha(&png) != ack.png_sha256
        || png.len() < 24
        || &png[..8] != b"\x89PNG\r\n\x1a\n"
        || u32::from_be_bytes(png[16..20].try_into().map_err(|_| "PNG width missing")?)
            != GROUP_ART_WIDTH
        || u32::from_be_bytes(png[20..24].try_into().map_err(|_| "PNG height missing")?)
            != GROUP_ART_HEIGHT
    {
        return Err("group PNG hash/extent differs from capture ACK".into());
    }
    let final_value = json!({"schema_version": 1, "profile": session.contract(),
        "status": "complete", "nonce": session.nonce(), "pid": std::process::id(),
        "probe_sha256": digest, "ack_sha256": sha(&bytes), "ack": ack,
        "stable_frames": frames, "completed_at_unix_ms": now_ms()?,
        "fixture": fixture, "snapshot": snapshot, "performance_evidence": false,
        "accepted": false, "promotion_authority": false});
    write_fresh(&final_path, &final_value)?;
    if std::fs::read(&ack_path).map_err(|e| e.to_string())? != bytes {
        return Err("group ACK changed during completion".into());
    }
    world.resource_mut::<CaptureState>().completed = true;
    world.write_message(AppExit::Success);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_requires_fresh_exact_process_probe_and_capture() {
        let session = GalleryCaptureSession::Group(
            crate::assets::building_asset_set::group_art::test_session(),
        );
        let ready = json!({"ready_at_unix_ms": 1000});
        let mut ack = CaptureAck {
            schema_version: 1,
            nonce: session.nonce().to_owned(),
            pid: std::process::id(),
            probe_sha256: "b".repeat(64),
            png_sha256: "c".repeat(64),
            window_id: "0x123".into(),
            width: GROUP_ART_WIDTH,
            height: GROUP_ART_HEIGHT,
            captured_at_unix_ms: 1500,
        };
        assert!(validate_ack(&ack, &session, &ready, &"b".repeat(64), 2000).is_ok());
        for bad_time in [999, 2001] {
            ack.captured_at_unix_ms = bad_time;
            assert!(validate_ack(&ack, &session, &ready, &"b".repeat(64), 2000).is_err());
        }
        ack.captured_at_unix_ms = 1500;
        assert!(validate_ack(&ack, &session, &ready, &"b".repeat(64), 32_000).is_err());
        assert!(validate_ack(&ack, &session, &ready, &"d".repeat(64), 2000).is_err());
        ack.pid += 1;
        assert!(validate_ack(&ack, &session, &ready, &"b".repeat(64), 2000).is_err());
        ack.pid = std::process::id();
        ack.window_id = "0x0".into();
        assert!(validate_ack(&ack, &session, &ready, &"b".repeat(64), 2000).is_err());
    }

    #[test]
    fn publication_refuses_replacing_a_previous_artifact() {
        let root =
            std::env::temp_dir().join(format!("group-gallery-publish-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("probe.json");
        let first = json!({"first": true});
        write_fresh(&path, &first).unwrap();
        assert!(write_fresh(&path, &json!({"second": true})).is_err());
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap(),
            first
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
