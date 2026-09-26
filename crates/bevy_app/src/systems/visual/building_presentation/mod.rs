//! Kind-local publication and presentation for the eight equipment sets.
//! No locators are loaded here: production admission remains explicit.

mod preview;
mod structure;

use crate::assets::building_asset_set::{
    BuildingAssetAuthority, BuildingAssetDescriptor, BuildingAssetKind, BuildingAssetPool,
    BuildingAssetSetManifest,
};
use bevy::prelude::*;
use hw_jobs::BuildingType;

pub use preview::sync_building_previews;
pub use structure::{EquipmentRoot, spawn_equipment_root, sync_equipment_structure};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingPresentationSet;

pub fn asset_kind(kind: BuildingType) -> Option<BuildingAssetKind> {
    Some(match kind {
        BuildingType::Tank => BuildingAssetKind::Tank,
        BuildingType::MudMixer => BuildingAssetKind::MudMixer,
        BuildingType::RestArea => BuildingAssetKind::RestArea,
        BuildingType::SoulSpa => BuildingAssetKind::SoulSpa,
        BuildingType::WheelbarrowParking => BuildingAssetKind::WheelbarrowParking,
        BuildingType::SandPile => BuildingAssetKind::SandPile,
        BuildingType::BonePile => BuildingAssetKind::BonePile,
        BuildingType::OutdoorLamp => BuildingAssetKind::OutdoorLamp,
        _ => return None,
    })
}

fn production(pool: &BuildingAssetPool, kind: BuildingType) -> Option<&BuildingAssetDescriptor> {
    let descriptor = pool.descriptor(asset_kind(kind)?)?;
    (descriptor.manifest.identity.authority == BuildingAssetAuthority::ReleaseApproved)
        .then_some(descriptor)
}

pub fn poll_building_assets(
    mut pool: ResMut<BuildingAssetPool>,
    server: Res<AssetServer>,
    manifests: Res<Assets<BuildingAssetSetManifest>>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
) {
    pool.poll(&server, &manifests, &meshes, &images);
}

#[cfg(test)]
mod tests;
