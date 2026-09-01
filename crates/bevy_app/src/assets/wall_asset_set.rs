//! Canonical runtime manifest and fail-closed loader for production Wall assets.

use std::fmt;

use std::sync::atomic::{AtomicU64, Ordering};

use bevy::asset::{AssetLoader, LoadContext, LoadState, io::Reader};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use hw_visual::TopDownStructuralMaterial;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const ASSET_SET_ID: &str = "wall-production-v1";
const CORE_INVENTORY: [(&str, &str); 8] = [
    ("models/buildings/wall/wall_isolated.glb", "mesh:isolated"),
    ("models/buildings/wall/wall_end.glb", "mesh:end"),
    ("models/buildings/wall/wall_straight.glb", "mesh:straight"),
    ("models/buildings/wall/wall_corner.glb", "mesh:corner"),
    (
        "models/buildings/wall/wall_t_junction.glb",
        "mesh:t_junction",
    ),
    ("models/buildings/wall/wall_cross.glb", "mesh:cross"),
    ("textures/buildings/wall/wall_albedo.png", "texture:albedo"),
    (
        "textures/buildings/wall/wall_emissive.png",
        "texture:emissive",
    ),
];
const NORMAL_PATH: &str = "textures/buildings/wall/wall_normal.png";
const WALLSET_PATH: &str = "manifests/wall-production-v1.wallset";
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallAssetAuthority {
    Candidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallNormalDecision {
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallAssetFileRecord {
    pub bytes: u64,
    pub path: String,
    pub role: String,
    pub sha256: String,
}

#[derive(Asset, TypePath, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallAssetSetManifest {
    pub asset_set_generation: u64,
    pub asset_set_id: String,
    pub authority: WallAssetAuthority,
    pub candidate_normal: WallAssetFileRecord,
    pub core: Vec<WallAssetFileRecord>,
    pub manifest_sha256: String,
    pub normal_decision: WallNormalDecision,
    pub receipt: Option<serde_json::Value>,
    pub schema_version: u32,
}

#[derive(Default, TypePath)]
pub struct WallAssetSetLoader;

#[derive(Debug)]
pub enum WallAssetSetLoadError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Contract(String),
}

impl fmt::Display for WallAssetSetLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not read Wall asset set: {error}"),
            Self::Json(error) => write!(formatter, "could not parse Wall asset set: {error}"),
            Self::Contract(message) => {
                write!(formatter, "Wall asset set contract failed: {message}")
            }
        }
    }
}

impl std::error::Error for WallAssetSetLoadError {}

impl From<std::io::Error> for WallAssetSetLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for WallAssetSetLoadError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

fn contract(condition: bool, message: impl Into<String>) -> Result<(), WallAssetSetLoadError> {
    if condition {
        Ok(())
    } else {
        Err(WallAssetSetLoadError::Contract(message.into()))
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn decode_canonical_wallset(
    bytes: &[u8],
) -> Result<WallAssetSetManifest, WallAssetSetLoadError> {
    let manifest: WallAssetSetManifest = serde_json::from_slice(bytes)?;
    let mut canonical = serde_json::to_vec(&manifest)?;
    canonical.push(b'\n');
    contract(bytes == canonical, "JSON bytes are not canonical")?;
    contract(manifest.schema_version == 1, "schema version differs")?;
    contract(
        manifest.asset_set_id == ASSET_SET_ID,
        "asset-set id differs",
    )?;
    contract(
        manifest.asset_set_generation > 0,
        "asset-set generation is zero",
    )?;
    contract(
        manifest.authority == WallAssetAuthority::Candidate,
        "candidate loader authority differs",
    )?;
    contract(
        manifest.normal_decision == WallNormalDecision::Pending,
        "candidate normal decision differs",
    )?;
    contract(manifest.receipt.is_none(), "candidate receipt must be null")?;
    contract(
        valid_sha256(&manifest.manifest_sha256),
        "manifest hash differs",
    )?;
    contract(
        manifest.core.len() == CORE_INVENTORY.len(),
        "core inventory length differs",
    )?;
    for (record, (path, role)) in manifest.core.iter().zip(CORE_INVENTORY) {
        validate_record(record, path, role)?;
    }
    validate_record(&manifest.candidate_normal, NORMAL_PATH, "texture:normal")?;
    Ok(manifest)
}

fn validate_record(
    record: &WallAssetFileRecord,
    path: &str,
    role: &str,
) -> Result<(), WallAssetSetLoadError> {
    contract(record.path == path, format!("{role} path differs"))?;
    contract(record.role == role, format!("{path} role differs"))?;
    contract(record.bytes > 0, format!("{path} byte length is zero"))?;
    contract(valid_sha256(&record.sha256), format!("{path} hash differs"))
}

impl AssetLoader for WallAssetSetLoader {
    type Asset = WallAssetSetManifest;
    type Settings = ();
    type Error = WallAssetSetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let manifest = decode_canonical_wallset(&bytes)?;
        for record in &manifest.core {
            let payload = load_context
                .read_asset_bytes(record.path.clone())
                .await
                .map_err(|error| WallAssetSetLoadError::Contract(error.to_string()))?;
            contract(
                payload.len() as u64 == record.bytes,
                format!("{} byte length differs", record.path),
            )?;
            let digest = format!("{:x}", Sha256::digest(&payload));
            contract(
                digest == record.sha256,
                format!("{} actual bytes hash differs", record.path),
            )?;
        }
        Ok(manifest)
    }

    fn extensions(&self) -> &[&str] {
        &["wallset"]
    }
}

#[derive(Resource, Clone)]
pub struct ProductionWallAssetPool {
    pub manifest: Handle<WallAssetSetManifest>,
    pub meshes: [Handle<Mesh>; 6],
    pub albedo: Handle<Image>,
    pub emissive: Handle<Image>,
    pub candidate_normal: Handle<Image>,
}

impl ProductionWallAssetPool {
    pub fn load(asset_server: &AssetServer) -> Self {
        let meshes = std::array::from_fn(|index| {
            let path = CORE_INVENTORY[index].0;
            asset_server.load(
                GltfAssetLabel::Primitive {
                    mesh: 0,
                    primitive: 0,
                }
                .from_asset(path),
            )
        });
        Self {
            manifest: asset_server.load(WALLSET_PATH),
            meshes,
            albedo: asset_server.load(CORE_INVENTORY[6].0),
            emissive: asset_server.load(CORE_INVENTORY[7].0),
            candidate_normal: asset_server.load(NORMAL_PATH),
        }
    }
}

#[derive(Resource, Clone)]
pub struct ProductionWallMaterialPool {
    pub complete: Handle<TopDownStructuralMaterial>,
    pub provisional: Handle<TopDownStructuralMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallAssetFallbackReason {
    CandidateDisabled,
    LoadFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallAssetReadinessState {
    Loading,
    Fallback(WallAssetFallbackReason),
    Eligible {
        asset_set_generation: u64,
        manifest_sha256: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallOptionalAssetState {
    Loading,
    Ready,
    Failed,
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct WallAssetReadiness {
    pub session_id: u64,
    pub activation_revision: u64,
    pub state: WallAssetReadinessState,
    pub candidate_normal: WallOptionalAssetState,
    pub candidate_normal_revision: u64,
}

impl Default for WallAssetReadiness {
    fn default() -> Self {
        Self {
            session_id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            activation_revision: 0,
            state: WallAssetReadinessState::Loading,
            candidate_normal: WallOptionalAssetState::Loading,
            candidate_normal_revision: 0,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallAssetCandidatePolicy {
    pub allow_candidate: bool,
}

impl Default for WallAssetCandidatePolicy {
    fn default() -> Self {
        Self {
            allow_candidate: std::env::var_os("HW_WALL_CANDIDATE")
                .is_some_and(|value| value == "1"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequiredLoadState {
    Loading,
    Ready,
    Failed,
}

fn required_load_state(
    asset_server: &AssetServer,
    id: impl Into<bevy::asset::UntypedAssetId>,
) -> RequiredLoadState {
    match asset_server.get_load_state(id) {
        Some(LoadState::Loaded) => RequiredLoadState::Ready,
        Some(LoadState::Failed(_)) => RequiredLoadState::Failed,
        Some(LoadState::NotLoaded | LoadState::Loading) | None => RequiredLoadState::Loading,
    }
}

fn aggregate_state(
    allow_candidate: bool,
    manifest: Option<&WallAssetSetManifest>,
    required: impl IntoIterator<Item = RequiredLoadState>,
) -> WallAssetReadinessState {
    let mut saw_loading = false;
    for state in required {
        match state {
            RequiredLoadState::Failed => {
                return WallAssetReadinessState::Fallback(WallAssetFallbackReason::LoadFailed);
            }
            RequiredLoadState::Loading => saw_loading = true,
            RequiredLoadState::Ready => {}
        }
    }
    if saw_loading || manifest.is_none() {
        return WallAssetReadinessState::Loading;
    }
    if !allow_candidate {
        return WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled);
    }
    let manifest = manifest.expect("manifest presence checked above");
    WallAssetReadinessState::Eligible {
        asset_set_generation: manifest.asset_set_generation,
        manifest_sha256: manifest.manifest_sha256.clone(),
    }
}

pub fn update_wall_asset_readiness_system(
    asset_server: Res<AssetServer>,
    manifests: Res<Assets<WallAssetSetManifest>>,
    pool: Res<ProductionWallAssetPool>,
    materials: Res<ProductionWallMaterialPool>,
    policy: Res<WallAssetCandidatePolicy>,
    mut readiness: ResMut<WallAssetReadiness>,
) {
    let _finite_material_pool = (materials.complete.id(), materials.provisional.id());
    let manifest_state = required_load_state(&asset_server, pool.manifest.id());
    let manifest = if manifest_state == RequiredLoadState::Ready {
        manifests.get(&pool.manifest)
    } else {
        None
    };
    let required = std::iter::once(manifest_state)
        .chain(
            pool.meshes
                .iter()
                .map(|handle| required_load_state(&asset_server, handle.id())),
        )
        .chain([
            required_load_state(&asset_server, pool.albedo.id()),
            required_load_state(&asset_server, pool.emissive.id()),
        ]);
    let next_state = aggregate_state(policy.allow_candidate, manifest, required);
    let next_normal = match required_load_state(&asset_server, pool.candidate_normal.id()) {
        RequiredLoadState::Loading => WallOptionalAssetState::Loading,
        RequiredLoadState::Ready => WallOptionalAssetState::Ready,
        RequiredLoadState::Failed => WallOptionalAssetState::Failed,
    };
    if readiness.state != next_state {
        readiness.state = next_state;
        readiness.activation_revision = readiness.activation_revision.wrapping_add(1);
    }
    if readiness.candidate_normal != next_normal {
        readiness.candidate_normal = next_normal;
        readiness.candidate_normal_revision = readiness.candidate_normal_revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::Duration;

    use bevy::asset::{AssetApp, AssetMetaCheck, AssetPlugin};

    use super::*;

    fn record(path: &str, role: &str) -> WallAssetFileRecord {
        WallAssetFileRecord {
            bytes: 1,
            path: path.to_string(),
            role: role.to_string(),
            sha256: "a".repeat(64),
        }
    }

    fn fixture() -> WallAssetSetManifest {
        WallAssetSetManifest {
            asset_set_generation: 1,
            asset_set_id: ASSET_SET_ID.to_string(),
            authority: WallAssetAuthority::Candidate,
            candidate_normal: record(NORMAL_PATH, "texture:normal"),
            core: CORE_INVENTORY
                .iter()
                .map(|(path, role)| record(path, role))
                .collect(),
            manifest_sha256: "b".repeat(64),
            normal_decision: WallNormalDecision::Pending,
            receipt: None,
            schema_version: 1,
        }
    }

    fn encoded(manifest: &WallAssetSetManifest) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(manifest).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn canonical_candidate_passes() {
        let manifest = fixture();
        assert_eq!(
            decode_canonical_wallset(&encoded(&manifest)).unwrap(),
            manifest
        );
    }

    #[test]
    fn pretty_or_reordered_json_is_rejected() {
        let bytes = serde_json::to_vec_pretty(&fixture()).unwrap();
        let error = decode_canonical_wallset(&bytes).unwrap_err();
        assert!(error.to_string().contains("not canonical"));
    }

    #[test]
    fn unknown_field_is_rejected() {
        let bytes = encoded(&fixture());
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unknown"] = serde_json::Value::Bool(true);
        let error = decode_canonical_wallset(&serde_json::to_vec(&value).unwrap()).unwrap_err();
        assert!(matches!(error, WallAssetSetLoadError::Json(_)));
    }

    #[test]
    fn swapped_mesh_role_is_rejected() {
        let mut manifest = fixture();
        manifest.core[0].role = "mesh:cross".to_string();
        let error = decode_canonical_wallset(&encoded(&manifest)).unwrap_err();
        assert!(error.to_string().contains("role differs"));
    }

    #[test]
    fn candidate_receipt_is_rejected() {
        let mut manifest = fixture();
        manifest.receipt = Some(serde_json::json!({"unexpected": true}));
        let error = decode_canonical_wallset(&encoded(&manifest)).unwrap_err();
        assert!(error.to_string().contains("receipt must be null"));
    }

    #[test]
    fn aggregate_is_all_or_nothing_and_candidate_gated() {
        let manifest = fixture();
        let states = [RequiredLoadState::Ready; 9];
        assert_eq!(
            aggregate_state(false, Some(&manifest), states),
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled)
        );
        assert!(matches!(
            aggregate_state(true, Some(&manifest), states),
            WallAssetReadinessState::Eligible { .. }
        ));
        let mut missing = states;
        missing[4] = RequiredLoadState::Loading;
        assert_eq!(
            aggregate_state(true, Some(&manifest), missing),
            WallAssetReadinessState::Loading
        );
        let mut failed = states;
        failed[7] = RequiredLoadState::Failed;
        assert_eq!(
            aggregate_state(true, Some(&manifest), failed),
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::LoadFailed)
        );
    }

    #[test]
    fn optional_normal_revision_is_separate_from_production_activation() {
        let mut readiness = WallAssetReadiness {
            state: WallAssetReadinessState::Eligible {
                asset_set_generation: 1,
                manifest_sha256: "b".repeat(64),
            },
            activation_revision: 4,
            candidate_normal: WallOptionalAssetState::Failed,
            candidate_normal_revision: 2,
            ..Default::default()
        };

        let next_normal = WallOptionalAssetState::Ready;
        if readiness.candidate_normal != next_normal {
            readiness.candidate_normal = next_normal;
            readiness.candidate_normal_revision =
                readiness.candidate_normal_revision.wrapping_add(1);
        }
        assert_eq!(readiness.activation_revision, 4);
        assert_eq!(readiness.candidate_normal_revision, 3);
    }

    struct TestAssetRoot(PathBuf);

    impl TestAssetRoot {
        fn new() -> Self {
            let ordinal = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "hell-workers-wallset-{}-{ordinal}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TestAssetRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn disk_fixture(root: &TestAssetRoot) -> WallAssetSetManifest {
        let mut manifest = fixture();
        for (ordinal, record) in manifest.core.iter_mut().enumerate() {
            let payload = format!("core-payload-{ordinal}");
            root.write(&record.path, payload.as_bytes());
            record.bytes = payload.len() as u64;
            record.sha256 = format!("{:x}", Sha256::digest(payload.as_bytes()));
        }
        root.write(NORMAL_PATH, b"optional-normal");
        manifest.candidate_normal.bytes = 15;
        manifest.candidate_normal.sha256 = format!("{:x}", Sha256::digest(b"optional-normal"));
        root.write(WALLSET_PATH, &encoded(&manifest));
        manifest
    }

    fn loader_app(root: &Path) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: root.to_string_lossy().into_owned(),
                watch_for_changes_override: Some(false),
                meta_check: AssetMetaCheck::Never,
                ..Default::default()
            },
        ))
        .init_asset::<WallAssetSetManifest>()
        .init_asset_loader::<WallAssetSetLoader>();
        app
    }

    fn wait_for_terminal_load(app: &mut App, handle: &Handle<WallAssetSetManifest>) -> LoadState {
        for _ in 0..200 {
            app.update();
            let state = app
                .world()
                .resource::<AssetServer>()
                .load_state(handle.id());
            if matches!(state, LoadState::Loaded | LoadState::Failed(_)) {
                return state;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("Wall asset set did not reach a terminal load state");
    }

    #[test]
    fn bevy_loader_reads_and_hashes_all_core_files_once() {
        let root = TestAssetRoot::new();
        let expected = disk_fixture(&root);
        let mut app = loader_app(&root.0);
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<WallAssetSetManifest>(WALLSET_PATH);
        assert!(matches!(
            wait_for_terminal_load(&mut app, &handle),
            LoadState::Loaded
        ));
        assert_eq!(
            app.world()
                .resource::<Assets<WallAssetSetManifest>>()
                .get(&handle),
            Some(&expected)
        );
    }

    #[test]
    fn bevy_loader_rejects_changed_core_bytes() {
        let root = TestAssetRoot::new();
        let manifest = disk_fixture(&root);
        root.write(&manifest.core[3].path, b"tampered");
        let mut app = loader_app(&root.0);
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<WallAssetSetManifest>(WALLSET_PATH);
        assert!(matches!(
            wait_for_terminal_load(&mut app, &handle),
            LoadState::Failed(_)
        ));
    }
}
