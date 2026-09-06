//! Canonical runtime manifest and fail-closed loader for production Door assets.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::asset::{AssetLoader, LoadContext, LoadState, io::Reader};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use hw_visual::{TopDownStructuralMaterial, make_topdown_structural_material};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const ASSET_SET_ID: &str = "door-production-v1";
const DOORSET_PATH: &str = "manifests/door-production-v1.doorset";
const CORE_INVENTORY: [(&str, &str); 6] = [
    ("models/buildings/door/door_closed.glb", "mesh:closed"),
    ("models/buildings/door/door_open.glb", "mesh:open"),
    ("models/buildings/door/door_locked.glb", "mesh:locked"),
    ("textures/buildings/door/door_albedo.png", "texture:albedo"),
    ("textures/buildings/door/door_preview_ew.png", "preview:ew"),
    ("textures/buildings/door/door_preview_ns.png", "preview:ns"),
];
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoorAssetAuthority {
    ArtPreview,
    IsolatedCandidate,
    ReleaseApproved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoorArtReviewStatus {
    ArtPreview,
    ArtApproved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoorAssetFileRecord {
    pub bytes: u64,
    pub path: String,
    pub role: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoorAssetReceiptRecord {
    pub bytes: u64,
    pub path: String,
    pub sha256: String,
}

#[derive(Asset, TypePath, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoorAssetSetManifest {
    pub asset_set_generation: u64,
    pub asset_set_id: String,
    pub authority: DoorAssetAuthority,
    pub core: Vec<DoorAssetFileRecord>,
    pub manifest_sha256: String,
    pub normal_decision: String,
    pub receipt: Option<DoorAssetReceiptRecord>,
    pub review_status: DoorArtReviewStatus,
    pub schema_version: u32,
}

#[derive(Default, TypePath)]
pub struct DoorAssetSetLoader;

#[derive(Debug)]
pub enum DoorAssetSetLoadError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Contract(String),
}

impl fmt::Display for DoorAssetSetLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not read Door asset set: {error}"),
            Self::Json(error) => write!(formatter, "could not parse Door asset set: {error}"),
            Self::Contract(message) => {
                write!(formatter, "Door asset set contract failed: {message}")
            }
        }
    }
}

impl std::error::Error for DoorAssetSetLoadError {}

impl From<std::io::Error> for DoorAssetSetLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for DoorAssetSetLoadError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

fn contract(condition: bool, message: impl Into<String>) -> Result<(), DoorAssetSetLoadError> {
    if condition {
        Ok(())
    } else {
        Err(DoorAssetSetLoadError::Contract(message.into()))
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn runtime_core_path(generation: u64, source_path: &str, role: &str) -> String {
    let relative = if role.starts_with("mesh:") {
        format!(
            "models/{}",
            source_path
                .rsplit_once('/')
                .map_or(source_path, |(_, name)| name)
        )
    } else {
        source_path.to_string()
    };
    format!("door_sets/{generation}/{relative}")
}

pub fn decode_canonical_doorset(
    bytes: &[u8],
) -> Result<DoorAssetSetManifest, DoorAssetSetLoadError> {
    let manifest: DoorAssetSetManifest = serde_json::from_slice(bytes)?;
    let mut canonical = serde_json::to_vec(&manifest)?;
    canonical.push(b'\n');
    contract(bytes == canonical, "JSON bytes are not canonical")?;
    contract(
        manifest.schema_version == 1 && manifest.asset_set_id == ASSET_SET_ID,
        "manifest identity differs",
    )?;
    contract(manifest.asset_set_generation > 0, "generation is zero")?;
    contract(
        valid_sha256(&manifest.manifest_sha256),
        "manifest hash differs",
    )?;
    contract(
        manifest.normal_decision == "not_used_by_design",
        "normal decision differs",
    )?;
    contract(
        manifest.core.len() == CORE_INVENTORY.len(),
        "core inventory length differs",
    )?;
    for (record, (path, role)) in manifest.core.iter().zip(CORE_INVENTORY) {
        let expected_path = match manifest.authority {
            DoorAssetAuthority::ArtPreview | DoorAssetAuthority::IsolatedCandidate => {
                path.to_string()
            }
            DoorAssetAuthority::ReleaseApproved => {
                runtime_core_path(manifest.asset_set_generation, path, role)
            }
        };
        contract(record.path == expected_path, format!("{role} path differs"))?;
        contract(record.role == role, format!("{path} role differs"))?;
        contract(record.bytes > 0, format!("{path} byte length is zero"))?;
        contract(valid_sha256(&record.sha256), format!("{path} hash differs"))?;
    }
    match manifest.authority {
        DoorAssetAuthority::ArtPreview => {
            contract(
                manifest.review_status == DoorArtReviewStatus::ArtPreview,
                "art preview review status differs",
            )?;
            contract(
                manifest.receipt.is_none(),
                "art preview receipt must be null",
            )?;
        }
        DoorAssetAuthority::IsolatedCandidate => {
            contract(
                manifest.review_status == DoorArtReviewStatus::ArtApproved,
                "candidate review status differs",
            )?;
            contract(manifest.receipt.is_none(), "candidate receipt must be null")?;
        }
        DoorAssetAuthority::ReleaseApproved => {
            contract(
                manifest.review_status == DoorArtReviewStatus::ArtApproved,
                "release review status differs",
            )?;
            let receipt = manifest.receipt.as_ref().ok_or_else(|| {
                DoorAssetSetLoadError::Contract("release receipt is absent".into())
            })?;
            contract(
                receipt.path
                    == format!(
                        "door_sets/{}/authority/promotion-receipt.json",
                        manifest.asset_set_generation
                    )
                    && receipt.bytes > 0
                    && valid_sha256(&receipt.sha256),
                "release receipt differs",
            )?;
        }
    }
    Ok(manifest)
}

impl AssetLoader for DoorAssetSetLoader {
    type Asset = DoorAssetSetManifest;
    type Settings = ();
    type Error = DoorAssetSetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let manifest = decode_canonical_doorset(&bytes)?;
        if let Some(receipt) = &manifest.receipt {
            let payload = load_context
                .read_asset_bytes(receipt.path.clone())
                .await
                .map_err(|error| DoorAssetSetLoadError::Contract(error.to_string()))?;
            contract(
                payload.len() as u64 == receipt.bytes,
                "receipt byte length differs",
            )?;
            contract(
                format!("{:x}", Sha256::digest(&payload)) == receipt.sha256,
                "receipt actual bytes hash differs",
            )?;
        }
        for record in &manifest.core {
            let payload = load_context
                .read_asset_bytes(record.path.clone())
                .await
                .map_err(|error| DoorAssetSetLoadError::Contract(error.to_string()))?;
            contract(
                payload.len() as u64 == record.bytes,
                format!("{} byte length differs", record.path),
            )?;
            contract(
                format!("{:x}", Sha256::digest(&payload)) == record.sha256,
                format!("{} actual bytes hash differs", record.path),
            )?;
        }
        Ok(manifest)
    }

    fn extensions(&self) -> &[&str] {
        &["doorset"]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoorAssetSetIdentity {
    pub asset_set_generation: u64,
    pub authority: DoorAssetAuthority,
    pub manifest_sha256: String,
}

impl From<&DoorAssetSetManifest> for DoorAssetSetIdentity {
    fn from(manifest: &DoorAssetSetManifest) -> Self {
        Self {
            asset_set_generation: manifest.asset_set_generation,
            authority: manifest.authority,
            manifest_sha256: manifest.manifest_sha256.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ResolvedProductionDoorAssets {
    pub identity: DoorAssetSetIdentity,
    pub meshes: [Handle<Mesh>; 3],
    pub albedo: Handle<Image>,
    pub preview_ew: Handle<Image>,
    pub preview_ns: Handle<Image>,
}

#[derive(Resource, Clone)]
pub struct ProductionDoorAssetPool {
    pub manifest: Handle<DoorAssetSetManifest>,
    pub resolved: Option<ResolvedProductionDoorAssets>,
}

impl ProductionDoorAssetPool {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            manifest: asset_server.load(DOORSET_PATH),
            resolved: None,
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct ProductionDoorMaterialPool {
    pub identity: Option<DoorAssetSetIdentity>,
    pub material: Option<Handle<TopDownStructuralMaterial>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorAssetFallbackReason {
    CandidateDisabled,
    LoadFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoorAssetReadinessState {
    Loading,
    Fallback(DoorAssetFallbackReason),
    Eligible(DoorAssetSetIdentity),
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct DoorAssetReadiness {
    pub session_id: u64,
    pub activation_revision: u64,
    pub state: DoorAssetReadinessState,
}

impl Default for DoorAssetReadiness {
    fn default() -> Self {
        Self {
            session_id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            activation_revision: 0,
            state: DoorAssetReadinessState::Loading,
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct DoorAssetCandidatePolicy {
    pub allow_art_preview: bool,
    pub allowed_generation: Option<u64>,
    pub allowed_manifest_sha256: Option<String>,
}

impl Default for DoorAssetCandidatePolicy {
    fn default() -> Self {
        let allow_art_preview = cfg!(feature = "profiling")
            && std::env::var_os("HW_DOOR_ART_PREVIEW").is_some_and(|value| value == "1");
        let enabled = allow_art_preview
            || std::env::var_os("HW_DOOR_CANDIDATE").is_some_and(|value| value == "1");
        let generation = std::env::var("HW_DOOR_CANDIDATE_GENERATION")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0);
        let manifest_sha256 = std::env::var("HW_DOOR_CANDIDATE_MANIFEST_SHA256")
            .ok()
            .filter(|value| valid_sha256(value));
        Self {
            allow_art_preview,
            allowed_generation: enabled.then_some(generation).flatten(),
            allowed_manifest_sha256: enabled.then_some(manifest_sha256).flatten(),
        }
    }
}

impl DoorAssetCandidatePolicy {
    fn allows(&self, manifest: &DoorAssetSetManifest) -> bool {
        (manifest.authority != DoorAssetAuthority::ArtPreview || self.allow_art_preview)
            && self.allowed_generation == Some(manifest.asset_set_generation)
            && self.allowed_manifest_sha256.as_deref() == Some(&manifest.manifest_sha256)
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

fn record_by_role<'a>(manifest: &'a DoorAssetSetManifest, role: &str) -> &'a DoorAssetFileRecord {
    manifest
        .core
        .iter()
        .find(|record| record.role == role)
        .unwrap_or_else(|| panic!("validated Door manifest lost role {role}"))
}

fn resolve_asset_handles(
    asset_server: &AssetServer,
    manifest: &DoorAssetSetManifest,
) -> ResolvedProductionDoorAssets {
    let meshes = std::array::from_fn(|index| {
        let record = record_by_role(manifest, CORE_INVENTORY[index].1);
        asset_server.load(
            GltfAssetLabel::Primitive {
                mesh: 0,
                primitive: 0,
            }
            .from_asset(record.path.clone()),
        )
    });
    ResolvedProductionDoorAssets {
        identity: manifest.into(),
        meshes,
        albedo: asset_server.load(record_by_role(manifest, "texture:albedo").path.clone()),
        preview_ew: asset_server.load(record_by_role(manifest, "preview:ew").path.clone()),
        preview_ns: asset_server.load(record_by_role(manifest, "preview:ns").path.clone()),
    }
}

fn update_state(readiness: &mut DoorAssetReadiness, state: DoorAssetReadinessState) {
    if readiness.state != state {
        readiness.state = state;
        readiness.activation_revision = readiness.activation_revision.wrapping_add(1);
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct DoorAssetReadinessParams<'w> {
    asset_server: Res<'w, AssetServer>,
    manifests: Res<'w, Assets<DoorAssetSetManifest>>,
    pool: ResMut<'w, ProductionDoorAssetPool>,
    materials: ResMut<'w, ProductionDoorMaterialPool>,
    structural_materials: ResMut<'w, Assets<TopDownStructuralMaterial>>,
    indoor_light: Res<'w, crate::systems::visual::indoor_light_texture::IndoorLightTexture>,
    policy: Res<'w, DoorAssetCandidatePolicy>,
    readiness: ResMut<'w, DoorAssetReadiness>,
}

pub fn update_door_asset_readiness_system(params: DoorAssetReadinessParams) {
    let DoorAssetReadinessParams {
        asset_server,
        manifests,
        mut pool,
        mut materials,
        mut structural_materials,
        indoor_light,
        policy,
        mut readiness,
    } = params;
    let manifest_state = required_load_state(&asset_server, pool.manifest.id());
    let manifest = (manifest_state == RequiredLoadState::Ready)
        .then(|| manifests.get(&pool.manifest))
        .flatten();
    if let Some(manifest) = manifest {
        let identity = DoorAssetSetIdentity::from(manifest);
        if pool.resolved.as_ref().map(|assets| &assets.identity) != Some(&identity) {
            pool.resolved = Some(resolve_asset_handles(&asset_server, manifest));
        }
        let resolved = pool.resolved.as_ref().expect("Door assets resolved above");
        if materials.identity.as_ref() != Some(&identity) {
            if let Some(old) = materials.material.take() {
                structural_materials.remove(old.id());
            }
            let mut material =
                make_topdown_structural_material(LinearRgba::WHITE, indoor_light.handle().clone());
            material.base.base_color_texture = Some(resolved.albedo.clone());
            materials.material = Some(structural_materials.add(material));
            materials.identity = Some(identity);
        }
    }
    if manifest_state == RequiredLoadState::Failed {
        update_state(
            &mut readiness,
            DoorAssetReadinessState::Fallback(DoorAssetFallbackReason::LoadFailed),
        );
        return;
    }
    let Some(manifest) = manifest else {
        update_state(&mut readiness, DoorAssetReadinessState::Loading);
        return;
    };
    let Some(resolved) = pool.resolved.as_ref() else {
        update_state(&mut readiness, DoorAssetReadinessState::Loading);
        return;
    };
    let required = resolved
        .meshes
        .iter()
        .map(|handle| required_load_state(&asset_server, handle.id()))
        .chain([
            required_load_state(&asset_server, resolved.albedo.id()),
            required_load_state(&asset_server, resolved.preview_ew.id()),
            required_load_state(&asset_server, resolved.preview_ns.id()),
        ])
        .chain(materials.material.iter().map(|_| RequiredLoadState::Ready));
    let mut saw_loading = false;
    for state in required {
        match state {
            RequiredLoadState::Failed => {
                update_state(
                    &mut readiness,
                    DoorAssetReadinessState::Fallback(DoorAssetFallbackReason::LoadFailed),
                );
                return;
            }
            RequiredLoadState::Loading => saw_loading = true,
            RequiredLoadState::Ready => {}
        }
    }
    if saw_loading || materials.material.is_none() {
        update_state(&mut readiness, DoorAssetReadinessState::Loading);
    } else if manifest.authority != DoorAssetAuthority::ReleaseApproved && !policy.allows(manifest)
    {
        update_state(
            &mut readiness,
            DoorAssetReadinessState::Fallback(DoorAssetFallbackReason::CandidateDisabled),
        );
    } else {
        update_state(
            &mut readiness,
            DoorAssetReadinessState::Eligible(manifest.into()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, role: &str) -> DoorAssetFileRecord {
        DoorAssetFileRecord {
            bytes: 1,
            path: path.to_string(),
            role: role.to_string(),
            sha256: "a".repeat(64),
        }
    }

    fn fixture(authority: DoorAssetAuthority) -> DoorAssetSetManifest {
        DoorAssetSetManifest {
            asset_set_generation: 1,
            asset_set_id: ASSET_SET_ID.to_string(),
            authority,
            core: CORE_INVENTORY
                .iter()
                .map(|(path, role)| record(path, role))
                .collect(),
            manifest_sha256: "b".repeat(64),
            normal_decision: "not_used_by_design".to_string(),
            receipt: None,
            review_status: if authority == DoorAssetAuthority::ArtPreview {
                DoorArtReviewStatus::ArtPreview
            } else {
                DoorArtReviewStatus::ArtApproved
            },
            schema_version: 1,
        }
    }

    fn canonical(manifest: &DoorAssetSetManifest) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(manifest).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn preview_and_candidate_require_distinct_review_authority() {
        assert!(
            decode_canonical_doorset(&canonical(&fixture(DoorAssetAuthority::ArtPreview))).is_ok()
        );
        assert!(
            decode_canonical_doorset(&canonical(&fixture(DoorAssetAuthority::IsolatedCandidate)))
                .is_ok()
        );
        let mut invalid = fixture(DoorAssetAuthority::ArtPreview);
        invalid.review_status = DoorArtReviewStatus::ArtApproved;
        assert!(decode_canonical_doorset(&canonical(&invalid)).is_err());
    }

    #[test]
    fn inventory_is_exact_and_canonical() {
        let mut missing = fixture(DoorAssetAuthority::IsolatedCandidate);
        missing.core.pop();
        assert!(decode_canonical_doorset(&canonical(&missing)).is_err());
        let pretty =
            serde_json::to_vec_pretty(&fixture(DoorAssetAuthority::IsolatedCandidate)).unwrap();
        assert!(decode_canonical_doorset(&pretty).is_err());
    }

    #[test]
    fn candidate_policy_requires_generation_and_hash() {
        let manifest = fixture(DoorAssetAuthority::IsolatedCandidate);
        let allowed = DoorAssetCandidatePolicy {
            allow_art_preview: false,
            allowed_generation: Some(1),
            allowed_manifest_sha256: Some("b".repeat(64)),
        };
        assert!(allowed.allows(&manifest));
        let mut preview = fixture(DoorAssetAuthority::ArtPreview);
        assert!(!allowed.allows(&preview));
        preview.authority = DoorAssetAuthority::IsolatedCandidate;
        preview.asset_set_generation = 2;
        assert!(!allowed.allows(&preview));
    }
}
