//! Spatial grid shared trait shim.
//!
//! Concrete grid本体は `hw_spatial` に移されたため、
//! root 側ではここから trait と型を再エクスポートして
//! 後方互換のインポート経路を維持する。

pub use hw_spatial::{GridData, SpatialGridOps};
