//! Shared, fail-closed input boundary for the nine equipment and Bridge asset sets.
//! Loading a manifest is not presentation activation or art approval.

#[cfg(feature = "profiling")]
pub mod acceptance;
mod loader;
#[cfg(feature = "profiling")]
mod observations;
mod pool;
mod projection;
mod release;
mod residency;
mod schema;
#[cfg(feature = "profiling")]
mod snapshot_io;
mod validation;

pub use loader::{BuildingAssetLoadPolicy, BuildingAssetSetLoader};
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
