//! WFC 生成後バリデータ（MS-WFC-2c）。
//!
//! - `lightweight_validate()`: 起動時必須チェック。失敗した試行は retry される。
//!   成功時は到達確認済み `ResourceSpawnCandidates` を返す。
//! - `debug_validate()`: `#[cfg(any(test, debug_assertions))]` で有効な追加診断。
//!   `Vec<ValidationWarning>` を返すだけで地形を変更しない。

#[cfg(any(test, debug_assertions))]
pub(crate) mod debug;
pub(crate) mod post_resource;
pub(crate) mod terrain;

use hw_core::world::GridPos;

// ── エラー / 警告型 ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ValidationError {
    ForbiddenTileInAnchorZone(GridPos),
    SiteYardNotReachable,
    RequiredResourceNotReachable,
    YardAnchorOutOfBounds(GridPos),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForbiddenTileInAnchorZone(pos) => {
                write!(f, "Site/Yard contains River or Sand at {pos:?}")
            }
            Self::SiteYardNotReachable => write!(f, "Site to Yard is not reachable"),
            Self::RequiredResourceNotReachable => {
                write!(f, "No required resource reachable from Yard")
            }
            Self::YardAnchorOutOfBounds(pos) => {
                write!(f, "Yard anchor not in Yard bounds: {pos:?}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug)]
pub struct ValidationWarning {
    pub kind: ValidationWarningKind,
    pub message: String,
}

#[derive(Debug)]
pub enum ValidationWarningKind {
    ProtectionBandViolation,
    RiverTileCountOutOfRange,
    FallbackReached,
    ForbiddenPattern,
    SandMaskMismatch,
}

// ── re-exports ────────────────────────────────────────────────────────────────

#[cfg(any(test, debug_assertions))]
pub use debug::debug_validate;
pub(crate) use post_resource::validate_post_resource;
pub use terrain::lightweight_validate;

// ── テスト ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen::generate_world_layout;
    use crate::terrain::TerrainType;
    use crate::test_seeds::GOLDEN_SEED_PRIMARY;
    use hw_core::constants::MAP_WIDTH;

    #[test]
    fn test_golden_seeds_pass_lightweight_validate() {
        for seed in [GOLDEN_SEED_PRIMARY] {
            let layout = generate_world_layout(seed);
            assert!(
                lightweight_validate(&layout).is_ok(),
                "seed={seed}: lightweight_validate failed"
            );
            assert!(
                !layout.resource_spawn_candidates.water_tiles.is_empty(),
                "seed={seed}: validated water_tiles missing"
            );
            assert!(
                !layout.resource_spawn_candidates.sand_tiles.is_empty(),
                "seed={seed}: validated sand_tiles missing"
            );
        }
    }

    #[test]
    fn test_fake_invalid_layout_fails_validate() {
        let mut layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
        // Site の左上角を River に書き換える
        let min_x = layout.anchors.site.min_x;
        let min_y = layout.anchors.site.min_y;
        let idx = (min_y * MAP_WIDTH + min_x) as usize;
        layout.terrain_tiles[idx] = TerrainType::River;
        assert!(lightweight_validate(&layout).is_err());
    }
}
