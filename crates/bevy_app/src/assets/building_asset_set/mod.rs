//! Shared, fail-closed input boundary for the eight non-Bridge building sets.
//! Loading a manifest is not presentation activation or art approval.

mod loader;
mod schema;
mod validation;

pub use loader::{BuildingAssetLoadPolicy, BuildingAssetSetLoader};
pub use schema::*;
pub use validation::{BuildingAssetSetError, decode_buildingset};

#[cfg(test)]
mod tests;
