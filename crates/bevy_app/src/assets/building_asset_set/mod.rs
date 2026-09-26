//! Shared, fail-closed input boundary for the eight non-Bridge building sets.
//! Loading a manifest is not presentation activation or art approval.

mod loader;
mod pool;
mod residency;
mod schema;
mod validation;

pub use loader::{BuildingAssetLoadPolicy, BuildingAssetSetLoader};
pub use pool::{BuildingAssetDescriptor, BuildingAssetPool};
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
