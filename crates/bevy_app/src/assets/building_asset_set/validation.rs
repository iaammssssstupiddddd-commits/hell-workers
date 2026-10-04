use std::fmt;

use hw_infra::lighting::digest_hex;
use sha2::{Digest, Sha256};

use super::schema::*;

#[derive(Debug)]
pub struct BuildingAssetSetError(pub(super) String);

impl fmt::Display for BuildingAssetSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BuildingAssetSetError {}

impl From<serde_json::Error> for BuildingAssetSetError {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<std::io::Error> for BuildingAssetSetError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

pub(super) fn require(value: bool, message: &str) -> Result<(), BuildingAssetSetError> {
    if value {
        Ok(())
    } else {
        Err(BuildingAssetSetError(message.into()))
    }
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

pub(super) fn digest(bytes: &[u8]) -> String {
    digest_hex(&Sha256::digest(bytes).into())
}

/// Receipt is excluded to avoid a circular manifest/receipt digest. Its own
/// bytes are content-addressed and its body must bind this exact identity.
pub(super) fn manifest_digest(
    manifest: &BuildingAssetSetManifest,
) -> Result<String, BuildingAssetSetError> {
    let mut content = manifest.clone();
    content.identity.manifest_sha256.clear();
    content.receipt = None;
    Ok(digest(&serde_json::to_vec(&content)?))
}

pub(super) fn canonical<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, BuildingAssetSetError> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_record(
    record: &BuildingArtifact,
    identity: &BuildingAssetSetIdentity,
    extension: &str,
) -> Result<(), BuildingAssetSetError> {
    require(
        record.bytes > 0 && valid_hash(&record.sha256),
        "invalid artifact size/hash",
    )?;
    // Exact equality excludes traversal, absolute paths, URL sources, labels,
    // and cross-kind/generation aliases; equal-content image roles may alias.
    let expected = format!(
        "building_sets/{}/{}/{}.{}",
        identity.kind.slug(),
        identity.generation,
        record.sha256,
        extension
    );
    require(
        record.path == expected,
        "artifact path is not immutable kind/generation/content address",
    )
}

fn validate_preview(
    preview: &BuildingAssetPreview,
    kind: BuildingAssetKind,
    catalog: bool,
) -> Result<(), BuildingAssetSetError> {
    let role = if catalog {
        "catalog"
    } else {
        kind.world_preview_role()
    };
    require(
        preview.image_role == role && preview.representative_state == kind.representative_state(),
        "preview role/state differs",
    )?;
    require(
        preview.canvas_px.iter().all(|v| *v > 0),
        "empty preview canvas",
    )?;
    require(
        preview.canvas_wu.iter().all(|v| v.is_finite() && *v > 0.0),
        "invalid preview world size",
    )?;
    require(
        preview
            .anchor_px
            .iter()
            .zip(preview.canvas_px)
            .all(|(anchor, extent)| {
                anchor.is_finite() && *anchor >= 0.0 && *anchor <= extent as f32
            }),
        "preview anchor outside canvas",
    )?;
    if catalog {
        require(
            preview.canvas_px[0] == preview.canvas_px[1]
                && preview.canvas_wu[0] == preview.canvas_wu[1],
            "catalog canvas must be square",
        )?;
        require(
            preview.anchor_px == preview.canvas_px.map(|v| v as f32 / 2.0),
            "catalog must be centered",
        )?;
    }
    Ok(())
}

pub fn decode_buildingset(bytes: &[u8]) -> Result<BuildingAssetSetManifest, BuildingAssetSetError> {
    let manifest: BuildingAssetSetManifest = serde_json::from_slice(bytes)?;
    require(
        bytes == canonical(&manifest)?,
        "buildingset JSON is not canonical",
    )?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &BuildingAssetSetManifest) -> Result<(), BuildingAssetSetError> {
    let identity = &manifest.identity;
    let kind = identity.kind;
    require(
        manifest.schema_version == 1 && identity.generation > 0,
        "unsupported schema or zero generation",
    )?;
    require(
        manifest.asset_set_id == format!("building-{}-v1", kind.slug()),
        "asset set/kind mismatch",
    )?;
    for hash in [
        &identity.manifest_sha256,
        &manifest.source_sha256,
        &manifest.export_sha256,
        &manifest.geometry_contract_sha256,
    ] {
        require(valid_hash(hash), "invalid identity hash")?;
    }
    require(
        identity.manifest_sha256 == manifest_digest(manifest)?,
        "manifest content hash mismatch",
    )?;
    let roles = kind
        .mesh_roles()
        .iter()
        .map(|role| (format!("mesh:{role}"), "glb"))
        .chain(
            kind.image_roles()
                .iter()
                .map(|role| (format!("image:{role}"), "png")),
        )
        .collect::<Vec<_>>();
    require(
        manifest.artifacts.len() == roles.len(),
        "artifact inventory length differs",
    )?;
    for (record, (role, extension)) in manifest.artifacts.iter().zip(roles) {
        require(
            record.role == role,
            "missing, duplicate, reordered or unknown artifact role",
        )?;
        validate_record(record, identity, extension)?;
    }
    let part_roles: &[(&str, &str)] = match kind {
        BuildingAssetKind::Tank => &[("body", "body"), ("water", "water")],
        BuildingAssetKind::MudMixer => &[("body", "body"), ("rotor", "rotor")],
        BuildingAssetKind::RestArea | BuildingAssetKind::Bridge => &[("body", "body")],
        BuildingAssetKind::SoulSpa => &[
            ("body", "body"),
            ("slot0", "slot"),
            ("slot1", "slot"),
            ("slot2", "slot"),
            ("slot3", "slot"),
        ],
        _ => &[],
    };
    require(
        manifest.parts.len() == part_roles.len(),
        "part inventory length differs",
    )?;
    for (part, (name, mesh)) in manifest.parts.iter().zip(part_roles) {
        require(
            part.name == *name && part.mesh_role == *mesh,
            "part name/mesh role differs",
        )?;
        let material = if *mesh == "slot" {
            BuildingPartMaterial::SpaSlot
        } else {
            BuildingPartMaterial::OpaqueAlbedo
        };
        require(part.material_role == material, "part material role differs")?;
        require(
            part.translation_wu.iter().all(|v| v.is_finite())
                && part.scale.iter().all(|v| v.is_finite() && *v > 0.0),
            "invalid part transform",
        )?;
        let length_squared: f32 = part.rotation_xyzw.iter().map(|v| v * v).sum();
        require(
            length_squared.is_finite() && (length_squared - 1.0).abs() <= 0.0001,
            "part quaternion is not normalized",
        )?;
    }
    validate_production_state(manifest)?;
    validate_preview(&manifest.world_preview, kind, false)?;
    validate_preview(&manifest.catalog_preview, kind, true)?;
    match identity.authority {
        BuildingAssetAuthority::ArtPreview => require(
            manifest.art_approval_sha256.is_none() && manifest.receipt.is_none(),
            "art preview cannot claim approval/release",
        )?,
        BuildingAssetAuthority::IsolatedCandidate | BuildingAssetAuthority::ReleaseApproved => {
            require(
                manifest
                    .art_approval_sha256
                    .as_deref()
                    .is_some_and(valid_hash),
                "art approval identity missing",
            )?;
            if identity.authority == BuildingAssetAuthority::IsolatedCandidate {
                require(manifest.receipt.is_none(), "candidate cannot claim release")?;
            } else {
                let record = manifest
                    .receipt
                    .as_ref()
                    .ok_or_else(|| BuildingAssetSetError("release receipt missing".into()))?;
                require(record.role == "authority:receipt", "receipt role differs")?;
                validate_record(record, identity, "json")?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_payload(
    record: &BuildingArtifact,
    bytes: &[u8],
) -> Result<(), BuildingAssetSetError> {
    require(
        bytes.len() as u64 == record.bytes && digest(bytes) == record.sha256,
        "artifact bytes/hash mismatch",
    )
}

pub(super) fn validate_receipt(
    manifest: &BuildingAssetSetManifest,
    bytes: &[u8],
) -> Result<(), BuildingAssetSetError> {
    let receipt: BuildingPromotionReceipt = serde_json::from_slice(bytes)?;
    require(
        bytes == canonical(&receipt)?,
        "receipt JSON is not canonical",
    )?;
    require(
        receipt.schema_version == 1
            && receipt.identity == manifest.identity
            && Some(&receipt.art_approval_sha256) == manifest.art_approval_sha256.as_ref()
            && receipt.numeric_approval_sha256 == manifest.numeric_approval_sha256
            && receipt.decision == "release_approved",
        "receipt does not approve this exact set",
    )
}

fn validate_production_state(
    manifest: &BuildingAssetSetManifest,
) -> Result<(), BuildingAssetSetError> {
    use BuildingProductionState as State;
    let kind = manifest.identity.kind;
    if manifest.identity.authority == BuildingAssetAuthority::ArtPreview {
        return require(
            manifest.production_state.is_none() && manifest.numeric_approval_sha256.is_none(),
            "draft cannot claim production state",
        );
    }
    let m2 = matches!(kind, BuildingAssetKind::Tank | BuildingAssetKind::MudMixer);
    require(
        if m2 {
            manifest
                .numeric_approval_sha256
                .as_deref()
                .is_some_and(valid_hash)
        } else {
            manifest.numeric_approval_sha256.is_none()
        },
        "missing or inapplicable numeric approval digest",
    )?;
    let valid = match (kind, manifest.production_state) {
        (
            BuildingAssetKind::Tank,
            Some(State::Tank {
                partial_y_wu,
                full_y_wu,
            }),
        ) => {
            partial_y_wu.is_finite()
                && full_y_wu.is_finite()
                && partial_y_wu > 0.0
                && full_y_wu > partial_y_wu
                && full_y_wu <= 2.0 * hw_core::constants::TILE_SIZE
        }
        (
            BuildingAssetKind::MudMixer,
            Some(State::MudMixer {
                axis,
                radians_per_second,
            }),
        ) => {
            let norm: f32 = axis.iter().map(|v| v * v).sum();
            norm.is_finite()
                && (norm - 1.0).abs() <= 0.0001
                && radians_per_second.is_finite()
                && radians_per_second > 0.0
                && radians_per_second <= std::f32::consts::TAU
        }
        (BuildingAssetKind::Tank | BuildingAssetKind::MudMixer, _) => false,
        (_, None) => true,
        _ => false,
    };
    require(
        valid,
        "missing, mismatched or out-of-range production state contract",
    )
}
