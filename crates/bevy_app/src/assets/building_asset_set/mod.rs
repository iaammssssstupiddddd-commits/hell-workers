//! Shared, fail-closed input boundary for the nine equipment and Bridge asset sets.
//! Loading a manifest is not presentation activation or art approval.

#[cfg(feature = "profiling")]
pub mod acceptance;
#[cfg(feature = "profiling")]
mod candidate_comparison;
#[cfg(feature = "profiling")]
pub(crate) mod group_art;
#[cfg(feature = "profiling")]
mod group_comparison;
#[cfg(feature = "profiling")]
pub(crate) use group_comparison::Group as BuildingArtGroup;
mod loader;
#[cfg(feature = "profiling")]
mod observations;
#[cfg(feature = "profiling")]
mod observations_gpu;
#[cfg(feature = "profiling")]
mod observations_history;
#[cfg(feature = "profiling")]
mod observations_m2;
#[cfg(feature = "profiling")]
mod observations_rest;
#[cfg(feature = "profiling")]
mod observations_spa;
#[cfg(feature = "profiling")]
mod observations_ui;
#[cfg(feature = "profiling")]
pub(crate) use observations_ui::visible_rect as observed_ui_visible_rect;
#[cfg(feature = "profiling")]
mod ordinary_art;
#[cfg(feature = "profiling")]
mod ordinary_m5;
mod pool;
mod projection;
mod release;
mod residency;
mod schema;
#[cfg(feature = "profiling")]
mod snapshot_io;
mod validation;

pub use loader::{BuildingAssetLoadPolicy, BuildingAssetSetLoader};
#[cfg(feature = "profiling")]
pub(crate) fn snapshot_m2_observations(
    world: &mut bevy::prelude::World,
) -> Result<serde_json::Value, String> {
    observations_m2::snapshot(world)
}
#[cfg(feature = "profiling")]
pub(crate) use observations_rest::snapshot as snapshot_rest_observations;
#[cfg(feature = "profiling")]
pub(crate) use observations_spa::snapshot as snapshot_spa_observations;
#[cfg(feature = "profiling")]
pub(crate) use ordinary_art::m2_persistence_enabled;
#[cfg(all(test, feature = "profiling"))]
pub(crate) use ordinary_art::m2_persistence_test_session;
#[cfg(feature = "profiling")]
pub(crate) use ordinary_art::m5_kind_for_case;
#[cfg(feature = "profiling")]
pub(crate) fn snapshot_ordinary_art(
    world: &mut bevy::prelude::World,
) -> Result<serde_json::Value, String> {
    ordinary_art::snapshot(world)
}
pub use pool::{BuildingAssetDescriptor, BuildingAssetPool};
pub use projection::project_building_asset_json;
#[cfg(feature = "profiling")]
pub(crate) use release::M6Comparison;
pub use release::configure_building_asset_releases;
pub use schema::*;
pub use validation::{BuildingAssetSetError, decode_buildingset};

/// Admission result, not an assertion that the requested generation is ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingAssetRequest {
    Started,
    /// A generation is already preparing; it is never displaced implicitly.
    PendingOccupied,
    /// Generations are immutable and strictly increasing per kind, including failures.
    AlreadyAttempted,
}

#[derive(Debug)]
pub struct BuildingAssetPoolFailure {
    pub identity: BuildingAssetSetIdentity,
    pub reason: String,
}

#[cfg(test)]
mod tests;
