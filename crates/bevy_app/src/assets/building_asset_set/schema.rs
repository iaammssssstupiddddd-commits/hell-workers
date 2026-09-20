use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Bridge is deliberately absent until its separate placement/art work resumes.
/// Wall and Door retain their existing schemas and loaders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildingAssetKind {
    Tank,
    MudMixer,
    RestArea,
    SoulSpa,
    WheelbarrowParking,
    SandPile,
    BonePile,
    OutdoorLamp,
}

impl BuildingAssetKind {
    #[cfg(test)]
    pub const ALL: [Self; 8] = [
        Self::Tank,
        Self::MudMixer,
        Self::RestArea,
        Self::SoulSpa,
        Self::WheelbarrowParking,
        Self::SandPile,
        Self::BonePile,
        Self::OutdoorLamp,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Tank => "tank",
            Self::MudMixer => "mud-mixer",
            Self::RestArea => "rest-area",
            Self::SoulSpa => "soul-spa",
            Self::WheelbarrowParking => "wheelbarrow-parking",
            Self::SandPile => "sand-pile",
            Self::BonePile => "bone-pile",
            Self::OutdoorLamp => "outdoor-lamp",
        }
    }

    pub fn mesh_roles(self) -> &'static [&'static str] {
        match self {
            Self::Tank => &["body", "water"],
            Self::MudMixer => &["body", "rotor"],
            Self::RestArea => &["body"],
            Self::SoulSpa => &["body", "slot"],
            _ => &[],
        }
    }

    pub fn image_roles(self) -> &'static [&'static str] {
        match self {
            Self::Tank | Self::MudMixer | Self::RestArea => &["albedo", "world_preview", "catalog"],
            Self::SoulSpa => &["albedo", "slot_emissive", "world_preview", "catalog"],
            Self::OutdoorLamp => &["world_off", "world_on", "catalog"],
            _ => &["world", "catalog"],
        }
    }

    pub fn representative_state(self) -> &'static str {
        match self {
            Self::Tank | Self::RestArea => "Empty",
            Self::MudMixer => "IdleAngleZero",
            Self::SoulSpa => "OperationalMaskZero",
            Self::WheelbarrowParking => "WithoutVehicle",
            Self::SandPile | Self::BonePile => "Static",
            Self::OutdoorLamp => "Off",
        }
    }

    pub fn world_preview_role(self) -> &'static str {
        match self {
            Self::OutdoorLamp => "world_off",
            kind if kind.mesh_roles().is_empty() => "world",
            _ => "world_preview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingAssetAuthority {
    ArtPreview,
    IsolatedCandidate,
    ReleaseApproved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingAssetSetIdentity {
    pub kind: BuildingAssetKind,
    pub generation: u64,
    pub authority: BuildingAssetAuthority,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingArtifact {
    pub role: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingPartMaterial {
    OpaqueAlbedo,
    SpaSlot,
}

/// The GLB is authored around its local pivot, at identity. All translations
/// are already world units; consumers must not multiply by TILE_SIZE again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingAssetPart {
    pub name: String,
    pub mesh_role: String,
    pub material_role: BuildingPartMaterial,
    pub translation_wu: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingAssetPreview {
    pub image_role: String,
    pub canvas_px: [u32; 2],
    pub canvas_wu: [f32; 2],
    /// Pixel coordinates from the top-left, not a normalized Bevy Anchor.
    pub anchor_px: [f32; 2],
    pub representative_state: String,
}

#[derive(Asset, TypePath, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingAssetSetManifest {
    pub schema_version: u32,
    pub asset_set_id: String,
    pub identity: BuildingAssetSetIdentity,
    pub source_sha256: String,
    pub export_sha256: String,
    pub geometry_contract_sha256: String,
    pub art_approval_sha256: Option<String>,
    pub artifacts: Vec<BuildingArtifact>,
    pub parts: Vec<BuildingAssetPart>,
    pub world_preview: BuildingAssetPreview,
    pub catalog_preview: BuildingAssetPreview,
    pub receipt: Option<BuildingArtifact>,
}

/// Product-owned release evidence, not an approval minted by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingPromotionReceipt {
    pub schema_version: u32,
    pub identity: BuildingAssetSetIdentity,
    pub art_approval_sha256: String,
    pub decision: String,
}
