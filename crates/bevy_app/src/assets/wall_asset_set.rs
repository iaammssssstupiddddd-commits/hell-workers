//! Canonical runtime manifest and fail-closed loader for production Wall assets.

use std::fmt;

use std::sync::atomic::{AtomicU64, Ordering};

use bevy::asset::{AssetLoader, LoadContext, LoadState, io::Reader};
use bevy::ecs::system::SystemParam;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use hw_visual::TopDownStructuralMaterial;
use hw_visual::wall_connection::WallTopologyIndex;
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
    IsolatedCandidate,
    ReleaseApproved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallNormalDecision {
    Pending,
    Adopted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallArtReviewStatus {
    ArtApproved,
    Candidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallAssetFileRecord {
    pub bytes: u64,
    pub path: String,
    pub role: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallAssetReceiptRecord {
    pub bytes: u64,
    pub path: String,
    pub sha256: String,
}

#[derive(Asset, TypePath, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallAssetSetManifest {
    pub asset_set_generation: u64,
    pub asset_set_id: String,
    pub authority: WallAssetAuthority,
    pub candidate_normal: Option<WallAssetFileRecord>,
    pub core: Vec<WallAssetFileRecord>,
    pub manifest_sha256: String,
    pub normal_decision: WallNormalDecision,
    pub receipt: Option<WallAssetReceiptRecord>,
    pub review_status: WallArtReviewStatus,
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WallReceiptApproval {
    approved_at_utc: String,
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WallReceiptArtifact {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WallReceiptActiveIdentity {
    asset_set_generation: u64,
    asset_set_id: String,
    manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, untagged)]
enum WallReceiptPreviousActive {
    Absent {
        status: WallReceiptPreviousStatus,
    },
    Present {
        asset_set_generation: u64,
        manifest_sha256: String,
        receipt_sha256: String,
        sha256: String,
        status: WallReceiptPreviousStatus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WallReceiptPreviousStatus {
    Absent,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WallPromotionReceipt {
    approval: WallReceiptApproval,
    asset_set_generation: u64,
    asset_set_id: String,
    m5_evidence_bundle: WallReceiptArtifact,
    manifest_sha256: String,
    new_active: WallReceiptActiveIdentity,
    previous_active: WallReceiptPreviousActive,
    promotion_plan_sha256: String,
    receipt_id: String,
    schema_version: u32,
    tool_commit: String,
    tool_tree: String,
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

fn valid_git_oid(value: &str) -> bool {
    value.len() == 40
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
    format!("wall_sets/{generation}/{relative}")
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
        valid_sha256(&manifest.manifest_sha256),
        "manifest hash differs",
    )?;
    let expected_core_len = match manifest.normal_decision {
        WallNormalDecision::Adopted => CORE_INVENTORY.len() + 1,
        WallNormalDecision::Pending | WallNormalDecision::Rejected => CORE_INVENTORY.len(),
    };
    contract(
        manifest.core.len() == expected_core_len,
        "core inventory length differs",
    )?;
    for (record, (path, role)) in manifest.core.iter().zip(CORE_INVENTORY) {
        let expected_path = match manifest.authority {
            WallAssetAuthority::IsolatedCandidate => path.to_string(),
            WallAssetAuthority::ReleaseApproved => {
                runtime_core_path(manifest.asset_set_generation, path, role)
            }
        };
        validate_record(record, &expected_path, role)?;
    }
    match manifest.authority {
        WallAssetAuthority::IsolatedCandidate => {
            contract(
                manifest.normal_decision != WallNormalDecision::Pending,
                "candidate normal decision differs",
            )?;
            contract(
                manifest.review_status == WallArtReviewStatus::ArtApproved,
                "candidate review status differs",
            )?;
            contract(manifest.receipt.is_none(), "candidate receipt must be null")?;
            contract(
                manifest.candidate_normal.is_none(),
                "candidate normal must be null",
            )?;
            if manifest.normal_decision == WallNormalDecision::Adopted {
                let record = manifest
                    .core
                    .last()
                    .expect("adopted core length checked above");
                validate_record(record, NORMAL_PATH, "texture:normal")?;
            }
        }
        WallAssetAuthority::ReleaseApproved => {
            contract(
                manifest.normal_decision != WallNormalDecision::Pending,
                "release normal decision is pending",
            )?;
            contract(
                manifest.review_status == WallArtReviewStatus::ArtApproved,
                "release review status differs",
            )?;
            contract(
                manifest.candidate_normal.is_none(),
                "release candidate normal must be null",
            )?;
            let receipt = manifest.receipt.as_ref().ok_or_else(|| {
                WallAssetSetLoadError::Contract("release receipt is absent".into())
            })?;
            let expected_receipt = format!(
                "wall_sets/{}/authority/promotion-receipt.json",
                manifest.asset_set_generation
            );
            validate_receipt_record(receipt, &expected_receipt)?;
            if manifest.normal_decision == WallNormalDecision::Adopted {
                let record = manifest
                    .core
                    .last()
                    .expect("adopted core length checked above");
                validate_record(
                    record,
                    &runtime_core_path(
                        manifest.asset_set_generation,
                        NORMAL_PATH,
                        "texture:normal",
                    ),
                    "texture:normal",
                )?;
            }
        }
    }
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

fn validate_receipt_record(
    record: &WallAssetReceiptRecord,
    path: &str,
) -> Result<(), WallAssetSetLoadError> {
    contract(record.path == path, "receipt path differs")?;
    contract(record.bytes > 0, "receipt byte length is zero")?;
    contract(valid_sha256(&record.sha256), "receipt hash differs")
}

fn decode_canonical_receipt(
    bytes: &[u8],
    manifest: &WallAssetSetManifest,
) -> Result<WallPromotionReceipt, WallAssetSetLoadError> {
    let receipt: WallPromotionReceipt = serde_json::from_slice(bytes)?;
    let mut canonical = serde_json::to_vec(&receipt)?;
    canonical.push(b'\n');
    contract(
        bytes == canonical,
        "promotion receipt JSON is not canonical",
    )?;
    contract(
        receipt.schema_version == 1 && receipt.asset_set_id == ASSET_SET_ID,
        "promotion receipt identity differs",
    )?;
    contract(
        receipt.asset_set_generation == manifest.asset_set_generation
            && receipt.manifest_sha256 == manifest.manifest_sha256,
        "promotion receipt payload binding differs",
    )?;
    contract(
        receipt.new_active
            == (WallReceiptActiveIdentity {
                asset_set_generation: manifest.asset_set_generation,
                asset_set_id: ASSET_SET_ID.to_string(),
                manifest_sha256: manifest.manifest_sha256.clone(),
            }),
        "promotion receipt new identity differs",
    )?;
    contract(
        valid_sha256(&receipt.promotion_plan_sha256)
            && valid_sha256(&receipt.m5_evidence_bundle.sha256)
            && valid_sha256(&receipt.approval.sha256),
        "promotion receipt authority hash differs",
    )?;
    contract(
        receipt.m5_evidence_bundle.path == "evidence/m5-evidence-bundle.json"
            && receipt.approval.path == "evidence/release-approval.json"
            && !receipt.approval.approved_at_utc.is_empty(),
        "promotion receipt authority locator differs",
    )?;
    contract(
        (8..=128).contains(&receipt.receipt_id.len())
            && receipt.receipt_id.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
            }),
        "promotion receipt ID differs",
    )?;
    contract(
        valid_git_oid(&receipt.tool_commit) && valid_git_oid(&receipt.tool_tree),
        "promotion receipt tool identity differs",
    )?;
    match &receipt.previous_active {
        WallReceiptPreviousActive::Absent { status } => contract(
            *status == WallReceiptPreviousStatus::Absent,
            "promotion receipt absent preimage differs",
        )?,
        WallReceiptPreviousActive::Present {
            asset_set_generation,
            manifest_sha256,
            receipt_sha256,
            sha256,
            status,
        } => {
            contract(
                *status == WallReceiptPreviousStatus::Present
                    && *asset_set_generation > 0
                    && valid_sha256(manifest_sha256)
                    && valid_sha256(receipt_sha256)
                    && valid_sha256(sha256),
                "promotion receipt present preimage differs",
            )?;
        }
    }
    Ok(receipt)
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
        if let Some(receipt) = &manifest.receipt {
            let payload = load_context
                .read_asset_bytes(receipt.path.clone())
                .await
                .map_err(|error| WallAssetSetLoadError::Contract(error.to_string()))?;
            contract(
                payload.len() as u64 == receipt.bytes,
                "promotion receipt byte length differs",
            )?;
            contract(
                format!("{:x}", Sha256::digest(&payload)) == receipt.sha256,
                "promotion receipt actual bytes hash differs",
            )?;
            decode_canonical_receipt(&payload, &manifest)?;
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallAssetSetIdentity {
    pub asset_set_generation: u64,
    pub authority: WallAssetAuthority,
    pub manifest_sha256: String,
}

impl From<&WallAssetSetManifest> for WallAssetSetIdentity {
    fn from(manifest: &WallAssetSetManifest) -> Self {
        Self {
            asset_set_generation: manifest.asset_set_generation,
            authority: manifest.authority,
            manifest_sha256: manifest.manifest_sha256.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ResolvedProductionWallAssets {
    pub identity: WallAssetSetIdentity,
    pub meshes: [Handle<Mesh>; 6],
    pub albedo: Handle<Image>,
    pub emissive: Handle<Image>,
    pub normal: Option<Handle<Image>>,
}

#[derive(Resource, Clone)]
pub struct ProductionWallAssetPool {
    pub manifest: Handle<WallAssetSetManifest>,
    pub resolved: Option<ResolvedProductionWallAssets>,
}

impl ProductionWallAssetPool {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            manifest: asset_server.load(WALLSET_PATH),
            resolved: None,
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct ProductionWallMaterialPool {
    pub identity: Option<WallAssetSetIdentity>,
    pub complete: Option<Handle<TopDownStructuralMaterial>>,
    pub provisional: Option<Handle<TopDownStructuralMaterial>>,
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
        authority: WallAssetAuthority,
        manifest_sha256: String,
    },
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct WallAssetReadiness {
    pub session_id: u64,
    pub activation_revision: u64,
    pub state: WallAssetReadinessState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallProductionFallbackReason {
    Loading,
    CandidateDisabled,
    LoadFailed,
    TopologyUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallProductionActivationState {
    Fallback(WallProductionFallbackReason),
    ReadyToApply {
        asset_set_generation: u64,
        authority: WallAssetAuthority,
        manifest_sha256: String,
        asset_activation_revision: u64,
    },
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct WallProductionActivation {
    pub(crate) topology_ready: bool,
    pub decision_revision: u64,
    pub state: WallProductionActivationState,
}

impl Default for WallProductionActivation {
    fn default() -> Self {
        Self {
            topology_ready: false,
            decision_revision: 0,
            state: WallProductionActivationState::Fallback(WallProductionFallbackReason::Loading),
        }
    }
}

impl Default for WallAssetReadiness {
    fn default() -> Self {
        Self {
            session_id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            activation_revision: 0,
            state: WallAssetReadinessState::Loading,
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct WallAssetCandidatePolicy {
    pub allowed_generation: Option<u64>,
    pub allowed_manifest_sha256: Option<String>,
}

impl Default for WallAssetCandidatePolicy {
    fn default() -> Self {
        let enabled = std::env::var_os("HW_WALL_CANDIDATE").is_some_and(|value| value == "1");
        let generation = std::env::var("HW_WALL_CANDIDATE_GENERATION")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0);
        let manifest_sha256 = std::env::var("HW_WALL_CANDIDATE_MANIFEST_SHA256")
            .ok()
            .filter(|value| valid_sha256(value));
        Self {
            allowed_generation: enabled.then_some(generation).flatten(),
            allowed_manifest_sha256: enabled.then_some(manifest_sha256).flatten(),
        }
    }
}

impl WallAssetCandidatePolicy {
    fn allows(&self, manifest: &WallAssetSetManifest) -> bool {
        self.allowed_generation == Some(manifest.asset_set_generation)
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

fn aggregate_state(
    candidate_policy: &WallAssetCandidatePolicy,
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
    let manifest = manifest.expect("manifest presence checked above");
    if manifest.authority == WallAssetAuthority::IsolatedCandidate
        && !candidate_policy.allows(manifest)
    {
        return WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled);
    }
    WallAssetReadinessState::Eligible {
        asset_set_generation: manifest.asset_set_generation,
        authority: manifest.authority,
        manifest_sha256: manifest.manifest_sha256.clone(),
    }
}

fn record_by_role<'a>(manifest: &'a WallAssetSetManifest, role: &str) -> &'a WallAssetFileRecord {
    manifest
        .core
        .iter()
        .find(|record| record.role == role)
        .unwrap_or_else(|| panic!("validated Wall manifest lost role {role}"))
}

fn resolve_asset_handles(
    asset_server: &AssetServer,
    manifest: &WallAssetSetManifest,
) -> ResolvedProductionWallAssets {
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
    let normal = (manifest.normal_decision == WallNormalDecision::Adopted).then(|| {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut bevy::image::ImageLoaderSettings| {
                settings.is_srgb = false;
            })
            .load(record_by_role(manifest, "texture:normal").path.clone())
    });
    ResolvedProductionWallAssets {
        identity: manifest.into(),
        meshes,
        albedo: asset_server.load(record_by_role(manifest, "texture:albedo").path.clone()),
        emissive: asset_server.load(record_by_role(manifest, "texture:emissive").path.clone()),
        normal,
    }
}

fn initialize_material_handles(
    assets: &ResolvedProductionWallAssets,
    indoor_light_field: Handle<Image>,
    structural_materials: &mut Assets<TopDownStructuralMaterial>,
) -> ProductionWallMaterialPool {
    use hw_visual::{make_topdown_structural_material, with_topdown_alpha_mode};

    let mut complete =
        make_topdown_structural_material(LinearRgba::WHITE, indoor_light_field.clone());
    complete.base.base_color_texture = Some(assets.albedo.clone());
    complete.base.emissive = LinearRgba::WHITE;
    complete.base.emissive_texture = Some(assets.emissive.clone());
    complete.base.normal_map_texture = assets.normal.clone();
    let complete = structural_materials.add(complete);

    let mut provisional =
        make_topdown_structural_material(LinearRgba::new(1.0, 1.0, 1.0, 0.9), indoor_light_field);
    provisional.base.base_color_texture = Some(assets.albedo.clone());
    provisional.base.emissive = LinearRgba::WHITE;
    provisional.base.emissive_texture = Some(assets.emissive.clone());
    provisional.base.normal_map_texture = assets.normal.clone();
    let provisional =
        structural_materials.add(with_topdown_alpha_mode(provisional, AlphaMode::Blend));

    ProductionWallMaterialPool {
        identity: Some(assets.identity.clone()),
        complete: Some(complete),
        provisional: Some(provisional),
    }
}

fn refresh_material_handles(
    assets: &ResolvedProductionWallAssets,
    indoor_light_field: Handle<Image>,
    pool: &mut ProductionWallMaterialPool,
    structural_materials: &mut Assets<TopDownStructuralMaterial>,
) -> bool {
    if pool.identity.as_ref() == Some(&assets.identity) {
        return false;
    }
    if let Some(handle) = pool.complete.take() {
        structural_materials.remove(handle.id());
    }
    if let Some(handle) = pool.provisional.take() {
        structural_materials.remove(handle.id());
    }
    *pool = initialize_material_handles(assets, indoor_light_field, structural_materials);
    true
}

fn apply_readiness_transition(
    readiness: &mut WallAssetReadiness,
    next_state: WallAssetReadinessState,
) {
    if readiness.state != next_state {
        readiness.state = next_state;
        readiness.activation_revision = readiness.activation_revision.wrapping_add(1);
    }
}

fn production_activation_state(
    readiness: &WallAssetReadiness,
    topology_ready: bool,
) -> WallProductionActivationState {
    match &readiness.state {
        WallAssetReadinessState::Loading => {
            WallProductionActivationState::Fallback(WallProductionFallbackReason::Loading)
        }
        WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled) => {
            WallProductionActivationState::Fallback(WallProductionFallbackReason::CandidateDisabled)
        }
        WallAssetReadinessState::Fallback(WallAssetFallbackReason::LoadFailed) => {
            WallProductionActivationState::Fallback(WallProductionFallbackReason::LoadFailed)
        }
        WallAssetReadinessState::Eligible {
            asset_set_generation,
            authority,
            manifest_sha256,
        } if topology_ready => WallProductionActivationState::ReadyToApply {
            asset_set_generation: *asset_set_generation,
            authority: *authority,
            manifest_sha256: manifest_sha256.clone(),
            asset_activation_revision: readiness.activation_revision,
        },
        WallAssetReadinessState::Eligible { .. } => WallProductionActivationState::Fallback(
            WallProductionFallbackReason::TopologyUnavailable,
        ),
    }
}

fn refresh_production_activation(
    activation: &mut WallProductionActivation,
    readiness: &WallAssetReadiness,
) {
    let next_state = production_activation_state(readiness, activation.topology_ready);
    if activation.state != next_state {
        activation.state = next_state;
        activation.decision_revision = activation.decision_revision.wrapping_add(1);
    }
}

/// Publishes topology readiness after the resolver and refreshes the atomic
/// production/fallback decision in the same `PostUpdate`.
pub fn finalize_wall_production_activation_system(
    topology: Res<WallTopologyIndex>,
    readiness: Res<WallAssetReadiness>,
    mut activation: ResMut<WallProductionActivation>,
) {
    activation.topology_ready = topology.is_ready();
    refresh_production_activation(&mut activation, &readiness);
}

#[derive(SystemParam)]
pub struct WallAssetReadinessParams<'w> {
    asset_server: Res<'w, AssetServer>,
    manifests: Res<'w, Assets<WallAssetSetManifest>>,
    pool: ResMut<'w, ProductionWallAssetPool>,
    materials: ResMut<'w, ProductionWallMaterialPool>,
    structural_materials: ResMut<'w, Assets<TopDownStructuralMaterial>>,
    indoor_light: Res<'w, crate::systems::visual::indoor_light_texture::IndoorLightTexture>,
    policy: Res<'w, WallAssetCandidatePolicy>,
}

pub fn update_wall_asset_readiness_system(
    mut params: WallAssetReadinessParams,
    mut readiness: ResMut<WallAssetReadiness>,
    mut activation: ResMut<WallProductionActivation>,
) {
    let manifest_state = required_load_state(&params.asset_server, params.pool.manifest.id());
    let manifest = if manifest_state == RequiredLoadState::Ready {
        params.manifests.get(&params.pool.manifest)
    } else {
        None
    };
    if let Some(manifest) = manifest {
        let identity = WallAssetSetIdentity::from(manifest);
        if params.pool.resolved.as_ref().map(|assets| &assets.identity) != Some(&identity) {
            params.pool.resolved = Some(resolve_asset_handles(&params.asset_server, manifest));
        }
        let resolved = params
            .pool
            .resolved
            .as_ref()
            .expect("resolved Wall assets were initialized above");
        refresh_material_handles(
            resolved,
            params.indoor_light.handle().clone(),
            &mut params.materials,
            &mut params.structural_materials,
        );
    }
    let resolved = params.pool.resolved.as_ref();
    let material_state = if resolved.is_some()
        && params.materials.complete.is_some()
        && params.materials.provisional.is_some()
    {
        RequiredLoadState::Ready
    } else {
        RequiredLoadState::Loading
    };
    let required = std::iter::once(manifest_state)
        .chain(std::iter::once(material_state))
        .chain(resolved.into_iter().flat_map(|assets| {
            assets
                .meshes
                .iter()
                .map(|handle| required_load_state(&params.asset_server, handle.id()))
                .chain([
                    required_load_state(&params.asset_server, assets.albedo.id()),
                    required_load_state(&params.asset_server, assets.emissive.id()),
                ])
                .chain(
                    assets
                        .normal
                        .iter()
                        .map(|handle| required_load_state(&params.asset_server, handle.id())),
                )
        }));
    let next_state = aggregate_state(&params.policy, manifest, required);
    apply_readiness_transition(&mut readiness, next_state);
    refresh_production_activation(&mut activation, &readiness);
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::Duration;

    use bevy::asset::{AssetApp, AssetMetaCheck, AssetPlugin};
    use bevy::camera::primitives::MeshAabb;
    use bevy::ecs::system::RunSystemOnce;

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
            authority: WallAssetAuthority::IsolatedCandidate,
            candidate_normal: None,
            core: CORE_INVENTORY
                .iter()
                .map(|(path, role)| record(path, role))
                .collect(),
            manifest_sha256: "b".repeat(64),
            normal_decision: WallNormalDecision::Rejected,
            receipt: None,
            review_status: WallArtReviewStatus::ArtApproved,
            schema_version: 1,
        }
    }

    fn release_fixture() -> WallAssetSetManifest {
        WallAssetSetManifest {
            asset_set_generation: 7,
            asset_set_id: ASSET_SET_ID.to_string(),
            authority: WallAssetAuthority::ReleaseApproved,
            candidate_normal: None,
            core: CORE_INVENTORY
                .iter()
                .map(|(path, role)| record(&runtime_core_path(7, path, role), role))
                .collect(),
            manifest_sha256: "b".repeat(64),
            normal_decision: WallNormalDecision::Rejected,
            receipt: Some(WallAssetReceiptRecord {
                bytes: 1,
                path: "wall_sets/7/authority/promotion-receipt.json".to_string(),
                sha256: "c".repeat(64),
            }),
            review_status: WallArtReviewStatus::ArtApproved,
            schema_version: 1,
        }
    }

    fn adopted_release_fixture() -> WallAssetSetManifest {
        let mut manifest = release_fixture();
        manifest.normal_decision = WallNormalDecision::Adopted;
        manifest.core.push(record(
            &runtime_core_path(7, NORMAL_PATH, "texture:normal"),
            "texture:normal",
        ));
        manifest
    }

    fn promotion_receipt(manifest: &WallAssetSetManifest) -> WallPromotionReceipt {
        WallPromotionReceipt {
            approval: WallReceiptApproval {
                approved_at_utc: "2026-09-01T00:00:00Z".to_string(),
                path: "evidence/release-approval.json".to_string(),
                sha256: "1".repeat(64),
            },
            asset_set_generation: manifest.asset_set_generation,
            asset_set_id: ASSET_SET_ID.to_string(),
            m5_evidence_bundle: WallReceiptArtifact {
                path: "evidence/m5-evidence-bundle.json".to_string(),
                sha256: "2".repeat(64),
            },
            manifest_sha256: manifest.manifest_sha256.clone(),
            new_active: WallReceiptActiveIdentity {
                asset_set_generation: manifest.asset_set_generation,
                asset_set_id: ASSET_SET_ID.to_string(),
                manifest_sha256: manifest.manifest_sha256.clone(),
            },
            previous_active: WallReceiptPreviousActive::Absent {
                status: WallReceiptPreviousStatus::Absent,
            },
            promotion_plan_sha256: "3".repeat(64),
            receipt_id: "receipt-release-01".to_string(),
            schema_version: 1,
            tool_commit: "4".repeat(40),
            tool_tree: "5".repeat(40),
        }
    }

    fn encoded_receipt(receipt: &WallPromotionReceipt) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(receipt).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn candidate_policy(manifest: Option<&WallAssetSetManifest>) -> WallAssetCandidatePolicy {
        WallAssetCandidatePolicy {
            allowed_generation: manifest.map(|value| value.asset_set_generation),
            allowed_manifest_sha256: manifest.map(|value| value.manifest_sha256.clone()),
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
    fn canonical_release_passes_with_generation_scoped_paths() {
        let manifest = release_fixture();
        assert_eq!(
            decode_canonical_wallset(&encoded(&manifest)).unwrap(),
            manifest
        );
    }

    #[test]
    fn release_without_receipt_is_rejected() {
        let mut manifest = release_fixture();
        manifest.receipt = None;
        let error = decode_canonical_wallset(&encoded(&manifest)).unwrap_err();
        assert!(error.to_string().contains("receipt is absent"));
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
        manifest.receipt = Some(WallAssetReceiptRecord {
            bytes: 1,
            path: "unexpected".to_string(),
            sha256: "c".repeat(64),
        });
        let error = decode_canonical_wallset(&encoded(&manifest)).unwrap_err();
        assert!(error.to_string().contains("receipt must be null"));
    }

    #[test]
    fn pending_art_candidate_is_rejected_after_review_closes() {
        let mut manifest = fixture();
        manifest.normal_decision = WallNormalDecision::Pending;
        manifest.review_status = WallArtReviewStatus::Candidate;
        manifest.candidate_normal = Some(record(NORMAL_PATH, "texture:normal"));
        let error = decode_canonical_wallset(&encoded(&manifest)).unwrap_err();
        assert!(error.to_string().contains("normal decision differs"));
    }

    #[test]
    fn aggregate_is_all_or_nothing_and_candidate_gated() {
        let manifest = fixture();
        let states = [RequiredLoadState::Ready; 9];
        assert_eq!(
            aggregate_state(&candidate_policy(None), Some(&manifest), states),
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled)
        );
        assert!(matches!(
            aggregate_state(&candidate_policy(Some(&manifest)), Some(&manifest), states),
            WallAssetReadinessState::Eligible { .. }
        ));
        let mut wrong_hash_policy = candidate_policy(Some(&manifest));
        wrong_hash_policy.allowed_manifest_sha256 = Some("f".repeat(64));
        assert_eq!(
            aggregate_state(&wrong_hash_policy, Some(&manifest), states),
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled)
        );
        let mut release = fixture();
        release.authority = WallAssetAuthority::ReleaseApproved;
        assert!(matches!(
            aggregate_state(&candidate_policy(None), Some(&release), states),
            WallAssetReadinessState::Eligible { .. }
        ));
        let mut missing = states;
        missing[4] = RequiredLoadState::Loading;
        assert_eq!(
            aggregate_state(&candidate_policy(Some(&manifest)), Some(&manifest), missing),
            WallAssetReadinessState::Loading
        );
        let mut failed = states;
        failed[7] = RequiredLoadState::Failed;
        assert_eq!(
            aggregate_state(&candidate_policy(Some(&manifest)), Some(&manifest), failed),
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::LoadFailed)
        );
    }

    #[test]
    fn readiness_revisions_change_once_per_observable_transition() {
        let manifest = fixture();
        let eligible = WallAssetReadinessState::Eligible {
            asset_set_generation: manifest.asset_set_generation,
            authority: manifest.authority,
            manifest_sha256: manifest.manifest_sha256.clone(),
        };
        let mut readiness = WallAssetReadiness::default();
        let session_id = readiness.session_id;

        apply_readiness_transition(&mut readiness, WallAssetReadinessState::Loading);
        assert_eq!(readiness.activation_revision, 0);

        apply_readiness_transition(
            &mut readiness,
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled),
        );
        assert_eq!(readiness.activation_revision, 1);

        apply_readiness_transition(&mut readiness, eligible.clone());
        assert_eq!(readiness.activation_revision, 2);

        apply_readiness_transition(&mut readiness, eligible);
        assert_eq!(readiness.activation_revision, 2);

        apply_readiness_transition(
            &mut readiness,
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::LoadFailed),
        );
        assert_eq!(readiness.activation_revision, 3);
        assert_eq!(readiness.session_id, session_id);
    }

    #[test]
    fn production_activation_requires_topology_and_falls_back_on_asset_failure() {
        let manifest = fixture();
        let mut readiness = WallAssetReadiness {
            activation_revision: 8,
            state: WallAssetReadinessState::Eligible {
                asset_set_generation: manifest.asset_set_generation,
                authority: manifest.authority,
                manifest_sha256: manifest.manifest_sha256.clone(),
            },
            ..Default::default()
        };
        let mut activation = WallProductionActivation::default();

        refresh_production_activation(&mut activation, &readiness);
        assert_eq!(
            activation.state,
            WallProductionActivationState::Fallback(
                WallProductionFallbackReason::TopologyUnavailable
            )
        );
        assert_eq!(activation.decision_revision, 1);

        activation.topology_ready = true;
        refresh_production_activation(&mut activation, &readiness);
        assert_eq!(
            activation.state,
            WallProductionActivationState::ReadyToApply {
                asset_set_generation: manifest.asset_set_generation,
                authority: manifest.authority,
                manifest_sha256: manifest.manifest_sha256,
                asset_activation_revision: 8,
            }
        );
        assert_eq!(activation.decision_revision, 2);

        refresh_production_activation(&mut activation, &readiness);
        assert_eq!(activation.decision_revision, 2);

        readiness.activation_revision = 9;
        readiness.state = WallAssetReadinessState::Fallback(WallAssetFallbackReason::LoadFailed);
        refresh_production_activation(&mut activation, &readiness);
        assert_eq!(
            activation.state,
            WallProductionActivationState::Fallback(WallProductionFallbackReason::LoadFailed)
        );
        assert_eq!(activation.decision_revision, 3);
    }

    #[test]
    fn production_material_pair_is_finite_and_keeps_light_field_binding() {
        let mut images = Assets::<Image>::default();
        let albedo = images.add(Image::default());
        let emissive = images.add(Image::default());
        let normal = images.add(Image::default());
        let indoor_light = images.add(Image::default());
        let mut materials = Assets::<TopDownStructuralMaterial>::default();
        let mut pool = ProductionWallMaterialPool::default();
        let mut assets = ResolvedProductionWallAssets {
            identity: WallAssetSetIdentity::from(&fixture()),
            meshes: std::array::from_fn(|_| Handle::default()),
            albedo: albedo.clone(),
            emissive: emissive.clone(),
            normal: Some(normal.clone()),
        };

        assert!(refresh_material_handles(
            &assets,
            indoor_light.clone(),
            &mut pool,
            &mut materials,
        ));
        assert_eq!(materials.len(), 2);
        let first_complete = pool.complete.clone().expect("complete material handle");
        let first_provisional = pool
            .provisional
            .clone()
            .expect("provisional material handle");

        for handle in [&first_complete, &first_provisional] {
            let material = materials.get(handle).expect("production Wall material");
            assert_eq!(material.base.base_color_texture.as_ref(), Some(&albedo));
            assert_eq!(material.base.emissive_texture.as_ref(), Some(&emissive));
            assert_eq!(material.base.normal_map_texture.as_ref(), Some(&normal));
            assert!(!material.base.unlit);
            assert_eq!(
                material.extension.indoor_light_field.as_ref(),
                Some(&indoor_light)
            );
        }
        assert_eq!(
            materials
                .get(&first_complete)
                .expect("complete material")
                .base
                .alpha_mode,
            AlphaMode::Opaque
        );
        assert_eq!(
            materials
                .get(&first_provisional)
                .expect("provisional material")
                .base
                .alpha_mode,
            AlphaMode::Blend
        );

        assert!(!refresh_material_handles(
            &assets,
            indoor_light.clone(),
            &mut pool,
            &mut materials,
        ));
        assert_eq!(pool.complete.as_ref(), Some(&first_complete));
        assert_eq!(pool.provisional.as_ref(), Some(&first_provisional));
        assert_eq!(materials.len(), 2);

        assets.identity.asset_set_generation += 1;
        assert!(refresh_material_handles(
            &assets,
            indoor_light,
            &mut pool,
            &mut materials,
        ));
        assert_eq!(materials.len(), 2);
        assert!(materials.get(&first_complete).is_none());
        assert!(materials.get(&first_provisional).is_none());
        assert_ne!(pool.complete.as_ref(), Some(&first_complete));
        assert_ne!(pool.provisional.as_ref(), Some(&first_provisional));
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
        root.write(WALLSET_PATH, &encoded(&manifest));
        manifest
    }

    fn disk_release_fixture(root: &TestAssetRoot) -> WallAssetSetManifest {
        disk_release_fixture_from(root, release_fixture())
    }

    fn disk_release_fixture_from(
        root: &TestAssetRoot,
        mut manifest: WallAssetSetManifest,
    ) -> WallAssetSetManifest {
        for (ordinal, record) in manifest.core.iter_mut().enumerate() {
            let payload = format!("release-core-payload-{ordinal}");
            root.write(&record.path, payload.as_bytes());
            record.bytes = payload.len() as u64;
            record.sha256 = format!("{:x}", Sha256::digest(payload.as_bytes()));
        }
        let receipt_bytes = encoded_receipt(&promotion_receipt(&manifest));
        let receipt = manifest.receipt.as_mut().unwrap();
        receipt.bytes = receipt_bytes.len() as u64;
        receipt.sha256 = format!("{:x}", Sha256::digest(&receipt_bytes));
        root.write(&receipt.path, &receipt_bytes);
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

    fn isolated_asset_app(root: &Path) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: root.to_string_lossy().into_owned(),
                watch_for_changes_override: Some(false),
                meta_check: AssetMetaCheck::Never,
                ..Default::default()
            },
            bevy::world_serialization::WorldSerializationPlugin,
            bevy::mesh::MeshPlugin,
            bevy::image::ImagePlugin::default_nearest(),
            bevy::gltf::GltfPlugin::default(),
        ))
        .init_asset::<WallAssetSetManifest>()
        .init_asset_loader::<WallAssetSetLoader>();
        app.register_asset_loader(bevy::image::ImageLoader::new(
            bevy::image::CompressedImageFormats::NONE,
        ));
        while app.plugins_state() == bevy::app::PluginsState::Adding {
            bevy::tasks::tick_global_task_pools_on_main_thread();
        }
        app.finish();
        app.cleanup();
        app
    }

    fn wait_for_required_assets(app: &mut App, assets: &ResolvedProductionWallAssets) {
        for _ in 0..10_000 {
            app.update();
            let server = app.world().resource::<AssetServer>();
            let mut states = assets
                .meshes
                .iter()
                .map(|handle| server.load_state(handle.id()))
                .chain([
                    server.load_state(assets.albedo.id()),
                    server.load_state(assets.emissive.id()),
                ]);
            if states
                .clone()
                .all(|state| matches!(state, LoadState::Loaded))
            {
                return;
            }
            if states.any(|state| matches!(state, LoadState::Failed(_))) {
                panic!("isolated Wall asset failed to load");
            }
        }
        let server = app.world().resource::<AssetServer>();
        let diagnostics: Vec<_> = assets
            .meshes
            .iter()
            .map(|handle| {
                (
                    server.get_path(handle.id()).map(|path| path.to_string()),
                    server.get_load_states(handle.id()),
                )
            })
            .chain([
                (
                    server
                        .get_path(assets.albedo.id())
                        .map(|path| path.to_string()),
                    server.get_load_states(assets.albedo.id()),
                ),
                (
                    server
                        .get_path(assets.emissive.id())
                        .map(|path| path.to_string()),
                    server.get_load_states(assets.emissive.id()),
                ),
            ])
            .collect();
        panic!("isolated Wall assets did not become ready: {diagnostics:?}");
    }

    #[test]
    #[ignore = "requires a sealed candidate asset view outside the primary worktree"]
    fn isolated_candidate_loads_six_real_primitives_with_valid_runtime_bounds() {
        let root = PathBuf::from(
            std::env::var("HW_WALL_ASSET_TEST_ROOT")
                .expect("HW_WALL_ASSET_TEST_ROOT must name the isolated assets directory"),
        );
        let mut app = isolated_asset_app(&root);
        let wallset = app
            .world()
            .resource::<AssetServer>()
            .load::<WallAssetSetManifest>(WALLSET_PATH);
        assert!(matches!(
            wait_for_terminal_load(&mut app, &wallset),
            LoadState::Loaded
        ));
        let manifest = app
            .world()
            .resource::<Assets<WallAssetSetManifest>>()
            .get(&wallset)
            .unwrap()
            .clone();
        let assets = resolve_asset_handles(app.world().resource::<AssetServer>(), &manifest);
        wait_for_required_assets(&mut app, &assets);

        app.world_mut()
            .insert_resource(Assets::<TopDownStructuralMaterial>::default());
        app.world_mut()
            .run_system_once(
                crate::systems::visual::indoor_light_texture::init_indoor_light_texture_system,
            )
            .expect("initialize focused Indoor Light texture");
        app.world_mut().insert_resource(ProductionWallAssetPool {
            manifest: wallset.clone(),
            resolved: None,
        });
        app.world_mut()
            .insert_resource(ProductionWallMaterialPool::default());
        app.world_mut().insert_resource(WallAssetCandidatePolicy {
            allowed_generation: None,
            allowed_manifest_sha256: None,
        });
        app.world_mut()
            .insert_resource(WallAssetReadiness::default());
        app.world_mut()
            .insert_resource(WallProductionActivation::default());
        app.add_systems(Update, update_wall_asset_readiness_system);

        app.update();
        let unauthorized = app.world().resource::<WallAssetReadiness>();
        assert_eq!(
            unauthorized.state,
            WallAssetReadinessState::Fallback(WallAssetFallbackReason::CandidateDisabled)
        );
        assert_eq!(unauthorized.activation_revision, 1);
        assert_eq!(
            app.world().resource::<WallProductionActivation>().state,
            WallProductionActivationState::Fallback(
                WallProductionFallbackReason::CandidateDisabled
            )
        );
        app.world_mut()
            .insert_resource(candidate_policy(Some(&manifest)));
        app.update();
        let eligible = app.world().resource::<WallAssetReadiness>().clone();
        assert!(matches!(
            eligible.state,
            WallAssetReadinessState::Eligible {
                asset_set_generation,
                authority: WallAssetAuthority::IsolatedCandidate,
                ..
            } if asset_set_generation == manifest.asset_set_generation
        ));
        assert_eq!(eligible.activation_revision, 2);
        assert_eq!(
            app.world().resource::<WallProductionActivation>().state,
            WallProductionActivationState::Fallback(
                WallProductionFallbackReason::TopologyUnavailable
            )
        );
        let materials = app.world().resource::<ProductionWallMaterialPool>();
        let complete = materials.complete.clone().expect("complete material");
        let provisional = materials.provisional.clone().expect("provisional material");
        assert_eq!(
            app.world()
                .resource::<Assets<TopDownStructuralMaterial>>()
                .len(),
            2
        );

        app.world_mut()
            .resource_mut::<WallProductionActivation>()
            .topology_ready = true;
        app.update();
        assert_eq!(app.world().resource::<WallAssetReadiness>(), &eligible);
        let ready_to_apply = app.world().resource::<WallProductionActivation>().clone();
        assert!(matches!(
            ready_to_apply.state,
            WallProductionActivationState::ReadyToApply {
                asset_set_generation,
                authority: WallAssetAuthority::IsolatedCandidate,
                asset_activation_revision: 2,
                ..
            } if asset_set_generation == manifest.asset_set_generation
        ));
        let materials = app.world().resource::<ProductionWallMaterialPool>();
        assert_eq!(materials.complete.as_ref(), Some(&complete));
        assert_eq!(materials.provisional.as_ref(), Some(&provisional));
        assert_eq!(
            app.world()
                .resource::<Assets<TopDownStructuralMaterial>>()
                .len(),
            2
        );
        app.update();
        assert_eq!(
            app.world().resource::<WallProductionActivation>(),
            &ready_to_apply
        );

        let meshes = app.world().resource::<Assets<Mesh>>();
        for handle in &assets.meshes {
            let mesh = meshes.get(handle).expect("loaded primitive Mesh is absent");
            let bounds = mesh
                .compute_aabb()
                .expect("primitive has no position bounds");
            let minimum = bounds.center - bounds.half_extents;
            let maximum = bounds.center + bounds.half_extents;
            assert!(minimum.x >= -16.01 && maximum.x <= 16.01);
            assert!(minimum.z >= -16.01 && maximum.z <= 16.01);
            assert!((minimum.y + 16.0).abs() <= 0.01);
            assert!((maximum.y - 16.0).abs() <= 0.01);
            let index_count = mesh
                .indices()
                .map_or_else(|| mesh.count_vertices(), |indices| indices.len());
            assert!(index_count / 3 <= 350);
        }
        let images = app.world().resource::<Assets<Image>>();
        assert_eq!(
            images
                .get(&assets.albedo)
                .expect("loaded albedo Image is absent")
                .texture_descriptor
                .format,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            images
                .get(&assets.emissive)
                .expect("loaded emissive Image is absent")
                .texture_descriptor
                .format,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
        );
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

    #[test]
    fn missing_albedo_or_emissive_recovers_only_in_a_fresh_app() {
        for missing_index in [6, 7] {
            let root = TestAssetRoot::new();
            let expected = disk_fixture(&root);
            let missing = &expected.core[missing_index];
            fs::remove_file(root.0.join(&missing.path)).unwrap();
            let mut failed_app = loader_app(&root.0);
            let failed_handle = failed_app
                .world()
                .resource::<AssetServer>()
                .load::<WallAssetSetManifest>(WALLSET_PATH);
            assert!(matches!(
                wait_for_terminal_load(&mut failed_app, &failed_handle),
                LoadState::Failed(_)
            ));

            root.write(
                &missing.path,
                format!("core-payload-{missing_index}").as_bytes(),
            );
            let mut restarted_app = loader_app(&root.0);
            let restarted_handle = restarted_app
                .world()
                .resource::<AssetServer>()
                .load::<WallAssetSetManifest>(WALLSET_PATH);
            assert!(matches!(
                wait_for_terminal_load(&mut restarted_app, &restarted_handle),
                LoadState::Loaded
            ));
            assert_eq!(
                restarted_app
                    .world()
                    .resource::<Assets<WallAssetSetManifest>>()
                    .get(&restarted_handle)
                    .unwrap()
                    .asset_set_generation,
                expected.asset_set_generation
            );
        }
    }

    #[test]
    fn bevy_loader_accepts_release_only_with_bound_receipt_bytes() {
        let root = TestAssetRoot::new();
        let expected = disk_release_fixture(&root);
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
    fn bevy_loader_rejects_changed_release_receipt_bytes() {
        let root = TestAssetRoot::new();
        let manifest = disk_release_fixture(&root);
        let receipt = manifest.receipt.unwrap();
        root.write(&receipt.path, b"{}\n");
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

    #[test]
    fn adopted_normal_is_required_release_core() {
        let root = TestAssetRoot::new();
        let manifest = disk_release_fixture_from(&root, adopted_release_fixture());
        let normal = manifest.core.last().unwrap();
        fs::remove_file(root.0.join(&normal.path)).unwrap();
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

    #[test]
    fn release_receipt_for_another_generation_fails_after_hash_match() {
        let root = TestAssetRoot::new();
        let mut manifest = disk_release_fixture(&root);
        let mut receipt = promotion_receipt(&manifest);
        receipt.asset_set_generation += 1;
        receipt.new_active.asset_set_generation += 1;
        let receipt_bytes = encoded_receipt(&receipt);
        let receipt_record = manifest.receipt.as_mut().unwrap();
        receipt_record.bytes = receipt_bytes.len() as u64;
        receipt_record.sha256 = format!("{:x}", Sha256::digest(&receipt_bytes));
        root.write(&receipt_record.path, &receipt_bytes);
        root.write(WALLSET_PATH, &encoded(&manifest));
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

    #[test]
    fn receipt_for_another_manifest_is_rejected() {
        let manifest = release_fixture();
        let mut receipt = promotion_receipt(&manifest);
        receipt.manifest_sha256 = "f".repeat(64);
        let error = decode_canonical_receipt(&encoded_receipt(&receipt), &manifest).unwrap_err();
        assert!(error.to_string().contains("payload binding differs"));
    }
}
