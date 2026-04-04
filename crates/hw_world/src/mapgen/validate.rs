//! WFC 生成後バリデータ（MS-WFC-2c）。
//!
//! - `lightweight_validate()`: 起動時必須チェック。失敗した試行は retry される。
//!   成功時は到達確認済み `ResourceSpawnCandidates` を返す。
//! - `debug_validate()`: `#[cfg(any(test, debug_assertions))]` で有効な追加診断。
//!   `Vec<ValidationWarning>` を返すだけで地形を変更しない。

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;

use std::collections::HashSet;

use crate::mapgen::resources::ResourceLayout;
use crate::mapgen::types::{GeneratedWorldLayout, ResourceSpawnCandidates};
use crate::pathfinding::{PathWorld, PathfindingContext, can_reach_target};
use crate::terrain::TerrainType;

#[cfg(any(test, debug_assertions))]
use crate::mapgen::wfc_adapter::CARDINAL_DIRS;
#[cfg(any(test, debug_assertions))]
use crate::river::{RIVER_TOTAL_TILES_TARGET_MAX, RIVER_TOTAL_TILES_TARGET_MIN};

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

// ── ValidatorPathWorld（内部ヘルパー） ────────────────────────────────────────

/// validate.rs 内部専用。`terrain_tiles` スライスのみで PathWorld を実現する。
/// 扉コストは常に 0（マップ生成段階では扉エンティティが存在しない）。
struct ValidatorPathWorld<'a> {
    tiles: &'a [TerrainType],
}

impl PathWorld for ValidatorPathWorld<'_> {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        if !(0..MAP_WIDTH).contains(&x) || !(0..MAP_HEIGHT).contains(&y) {
            return None;
        }
        Some((y * MAP_WIDTH + x) as usize)
    }

    fn idx_to_pos(&self, idx: usize) -> GridPos {
        (idx as i32 % MAP_WIDTH, idx as i32 / MAP_WIDTH)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.pos_to_idx(x, y)
            .map(|i| self.tiles[i].is_walkable())
            .unwrap_or(false)
    }

    fn get_door_cost(&self, _x: i32, _y: i32) -> i32 {
        0
    }
}

// ── lightweight_validate ──────────────────────────────────────────────────────

/// 起動時必須チェック。失敗時は Err を返す。
/// 成功時は validator が確認した到達可能資源候補を返す。
/// retry / fallback / panic の判断は validator の外側で行う。
pub fn lightweight_validate(
    layout: &GeneratedWorldLayout,
) -> Result<ResourceSpawnCandidates, ValidationError> {
    check_site_yard_no_river_sand(layout)?;
    check_site_yard_reachable(layout)?;
    let resource_spawn_candidates = collect_required_resource_candidates(layout)?;
    check_yard_anchors_present(layout)?;
    Ok(resource_spawn_candidates)
}

fn check_site_yard_no_river_sand(layout: &GeneratedWorldLayout) -> Result<(), ValidationError> {
    for pos in layout
        .anchors
        .site
        .iter_cells()
        .chain(layout.anchors.yard.iter_cells())
    {
        let idx = (pos.1 * MAP_WIDTH + pos.0) as usize;
        let tile = layout.terrain_tiles[idx];
        if matches!(tile, TerrainType::River | TerrainType::Sand) {
            return Err(ValidationError::ForbiddenTileInAnchorZone(pos));
        }
    }
    Ok(())
}

fn check_site_yard_reachable(layout: &GeneratedWorldLayout) -> Result<(), ValidationError> {
    let world = ValidatorPathWorld {
        tiles: &layout.terrain_tiles,
    };
    let mut ctx = PathfindingContext::default();
    let site_rep = (layout.anchors.site.min_x, layout.anchors.site.min_y);
    let yard_rep = (layout.anchors.yard.min_x, layout.anchors.yard.min_y);
    if !can_reach_target(&world, &mut ctx, site_rep, yard_rep, true) {
        return Err(ValidationError::SiteYardNotReachable);
    }
    Ok(())
}

/// Yard 代表点から各資源への到達可能性を確認し、到達確認済み候補集合を返す。
///
/// River タイルは walkable=false のため `can_reach_target(..., false)` を使う
/// （内部で `find_path_to_adjacent` が呼ばれ、隣接到達を判定する）。
fn collect_required_resource_candidates(
    layout: &GeneratedWorldLayout,
) -> Result<ResourceSpawnCandidates, ValidationError> {
    let world = ValidatorPathWorld {
        tiles: &layout.terrain_tiles,
    };
    let mut ctx = PathfindingContext::default();
    let yard_rep = (layout.anchors.yard.min_x, layout.anchors.yard.min_y);

    let mut validated = ResourceSpawnCandidates {
        water_tiles: Vec::new(),
        sand_tiles: Vec::new(),
        rock_candidates: Vec::new(),
    };

    // 水源: mask と terrain の両方が River のセルのみ列挙し、隣接到達可能なものを保持する
    let river_tiles: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| {
                let idx = (y * MAP_WIDTH + x) as usize;
                let is_river_terrain = layout.terrain_tiles[idx] == TerrainType::River;
                (layout.masks.river_mask.get((x, y)) && is_river_terrain).then_some((x, y))
            })
        })
        .collect();
    validated.water_tiles = river_tiles
        .into_iter()
        .filter(|&pos| can_reach_target(&world, &mut ctx, yard_rep, pos, false))
        .collect();
    if validated.water_tiles.is_empty() {
        return Err(ValidationError::RequiredResourceNotReachable);
    }

    // 砂源: 各 Sand タイルを個別に到達確認し、到達可能なものだけ保持する
    let sand_tiles: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| {
                let idx = (y * MAP_WIDTH + x) as usize;
                (layout.terrain_tiles[idx] == TerrainType::Sand).then_some((x, y))
            })
        })
        .collect();
    validated.sand_tiles = sand_tiles
        .into_iter()
        .filter(|&pos| can_reach_target(&world, &mut ctx, yard_rep, pos, true))
        .collect();
    if validated.sand_tiles.is_empty() {
        return Err(ValidationError::RequiredResourceNotReachable);
    }

    // 岩源: 入力候補がある場合だけ到達可能なものを残す。1 件も残らなければ Err。
    if !layout.resource_spawn_candidates.rock_candidates.is_empty() {
        validated.rock_candidates = layout
            .resource_spawn_candidates
            .rock_candidates
            .iter()
            .copied()
            .filter(|&pos| can_reach_target(&world, &mut ctx, yard_rep, pos, true))
            .collect();
        if validated.rock_candidates.is_empty() {
            return Err(ValidationError::RequiredResourceNotReachable);
        }
    }

    Ok(validated)
}

fn check_yard_anchors_present(layout: &GeneratedWorldLayout) -> Result<(), ValidationError> {
    for &pos in &layout.anchors.initial_wood_positions {
        if !layout.anchors.yard.contains(pos) {
            return Err(ValidationError::YardAnchorOutOfBounds(pos));
        }
    }
    for pos in layout.anchors.wheelbarrow_parking.iter_cells() {
        if !layout.anchors.yard.contains(pos) {
            return Err(ValidationError::YardAnchorOutOfBounds(pos));
        }
    }
    Ok(())
}

// ── ResourceObstaclePathWorld（内部ヘルパー） ─────────────────────────────────

/// validate_post_resource 専用。TerrainType に加え、木・岩の障害物セットを重ねる。
struct ResourceObstaclePathWorld<'a> {
    tiles: &'a [TerrainType],
    obstacles: &'a HashSet<GridPos>,
}

impl PathWorld for ResourceObstaclePathWorld<'_> {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        if !(0..MAP_WIDTH).contains(&x) || !(0..MAP_HEIGHT).contains(&y) {
            return None;
        }
        Some((y * MAP_WIDTH + x) as usize)
    }

    fn idx_to_pos(&self, idx: usize) -> GridPos {
        (idx as i32 % MAP_WIDTH, idx as i32 / MAP_WIDTH)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        !self.obstacles.contains(&(x, y))
            && self
                .pos_to_idx(x, y)
                .map(|i| self.tiles[i].is_walkable())
                .unwrap_or(false)
    }

    fn get_door_cost(&self, _x: i32, _y: i32) -> i32 {
        0
    }
}

// ── validate_post_resource ────────────────────────────────────────────────────

/// 木・岩配置後の到達性確認。
///
/// `layout` は `lightweight_validate` 通過済み（`water_tiles` / `sand_tiles` が入っている）。
/// `resource` の木・岩座標を歩行不可障害物として重ね、
/// Site↔Yard、Yard→水源、Yard→砂源、Yard→岩（隣接）を再確認する。
///
/// 岩は障害物なので `can_reach_target(..., false)` で隣接到達を要求する点が
/// 地形フェーズの `collect_required_resource_candidates` と異なる（意図的）。
pub(crate) fn validate_post_resource(
    layout: &GeneratedWorldLayout,
    resource: &ResourceLayout,
) -> Result<(), ValidationError> {
    let mut obstacles: HashSet<GridPos> = HashSet::new();
    obstacles.extend(resource.initial_tree_positions.iter().copied());
    obstacles.extend(resource.initial_rock_positions.iter().copied());

    let world = ResourceObstaclePathWorld {
        tiles: &layout.terrain_tiles,
        obstacles: &obstacles,
    };
    let mut ctx = PathfindingContext::default();
    let site_rep = (layout.anchors.site.min_x, layout.anchors.site.min_y);
    let yard_rep = (layout.anchors.yard.min_x, layout.anchors.yard.min_y);

    if !can_reach_target(&world, &mut ctx, site_rep, yard_rep, true) {
        return Err(ValidationError::SiteYardNotReachable);
    }

    let has_water = layout
        .resource_spawn_candidates
        .water_tiles
        .iter()
        .any(|&p| can_reach_target(&world, &mut ctx, yard_rep, p, false));
    if !has_water {
        return Err(ValidationError::RequiredResourceNotReachable);
    }

    let has_sand = layout
        .resource_spawn_candidates
        .sand_tiles
        .iter()
        .any(|&p| can_reach_target(&world, &mut ctx, yard_rep, p, true));
    if !has_sand {
        return Err(ValidationError::RequiredResourceNotReachable);
    }

    let has_rock = resource
        .initial_rock_positions
        .iter()
        .any(|&p| can_reach_target(&world, &mut ctx, yard_rep, p, false));
    if !has_rock {
        return Err(ValidationError::RequiredResourceNotReachable);
    }

    Ok(())
}

// ── debug_validate ────────────────────────────────────────────────────────────

#[cfg(any(test, debug_assertions))]
pub fn debug_validate(layout: &GeneratedWorldLayout) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    check_protection_band_clean(layout, &mut warnings);
    check_river_tile_count(layout, &mut warnings);
    check_no_fallback_reached(layout, &mut warnings);
    check_forbidden_diagonal_patterns(layout, &mut warnings);
    check_final_sand_mask_applied(layout, &mut warnings);
    check_no_stray_sand_outside_mask(layout, &mut warnings);
    check_sand_mask_not_in_anchor_or_band(layout, &mut warnings);
    warnings
}

/// River タイルがアンカー外周保護帯（river_protection_band）に侵入していないか確認する。
#[cfg(any(test, debug_assertions))]
fn check_protection_band_clean(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.river_mask.get((x, y)) && layout.masks.river_protection_band.get((x, y))
            {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::ProtectionBandViolation,
                    message: format!("River at ({x},{y}) is inside river_protection_band"),
                });
            }
        }
    }
}

/// river_mask のセル数が RIVER_TOTAL_TILES_TARGET_MIN/MAX の範囲外なら警告する。
#[cfg(any(test, debug_assertions))]
fn check_river_tile_count(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    let count = layout.masks.river_mask.count_set();
    if !(RIVER_TOTAL_TILES_TARGET_MIN..=RIVER_TOTAL_TILES_TARGET_MAX).contains(&count) {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::RiverTileCountOutOfRange,
            message: format!(
                "River tile count {count} outside [{RIVER_TOTAL_TILES_TARGET_MIN}, {RIVER_TOTAL_TILES_TARGET_MAX}]"
            ),
        });
    }
}

/// fallback 地形が使われた場合に警告する（debug_assert! は使わない）。
#[cfg(any(test, debug_assertions))]
fn check_no_fallback_reached(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    if layout.used_fallback {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::FallbackReached,
            message: format!(
                "WFC fallback terrain used (master_seed={}, attempt={})",
                layout.master_seed, layout.generation_attempt
            ),
        });
    }
}

/// 4 近傍に River 隣接がない孤立 River タイルを検出する（F2 斜め整合診断の一部）。
#[cfg(any(test, debug_assertions))]
fn check_forbidden_diagonal_patterns(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if layout.terrain_tiles[idx] != TerrainType::River {
                continue;
            }
            let has_river_neighbor = CARDINAL_DIRS.iter().any(|(dx, dy)| {
                let nx = x + dx;
                let ny = y + dy;
                if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                    return false;
                }
                layout.terrain_tiles[(ny * MAP_WIDTH + nx) as usize] == TerrainType::River
            });
            if !has_river_neighbor {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::ForbiddenPattern,
                    message: format!(
                        "Isolated River tile (no cardinal River neighbor) at ({x},{y})"
                    ),
                });
            }
        }
    }
}

/// final_sand_mask == true のセルがすべて TerrainType::Sand になっているか確認する。
#[cfg(any(test, debug_assertions))]
fn check_final_sand_mask_applied(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.final_sand_mask.get((x, y)) {
                let idx = (y * MAP_WIDTH + x) as usize;
                if layout.terrain_tiles[idx] != TerrainType::Sand {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::SandMaskMismatch,
                        message: format!(
                            "final_sand_mask=true but terrain != Sand at ({x},{y}): {:?}",
                            layout.terrain_tiles[idx]
                        ),
                    });
                }
            }
        }
    }
}

/// final_sand_mask または inland_sand_mask の外に TerrainType::Sand が残っていないか確認する。
#[cfg(any(test, debug_assertions))]
fn check_no_stray_sand_outside_mask(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let pos = (x, y);
            // final_sand_mask と inland_sand_mask の合法領域を合わせて判定
            let in_legal_sand =
                layout.masks.final_sand_mask.get(pos) || layout.masks.inland_sand_mask.get(pos);
            if !in_legal_sand {
                let idx = (y * MAP_WIDTH + x) as usize;
                if layout.terrain_tiles[idx] == TerrainType::Sand {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::SandMaskMismatch,
                        message: format!("Stray Sand outside sand masks at ({x},{y})"),
                    });
                }
            }
        }
    }
}

/// final_sand_mask が anchor_mask / river_protection_band と交差しないか確認する。
#[cfg(any(test, debug_assertions))]
fn check_sand_mask_not_in_anchor_or_band(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if !layout.masks.final_sand_mask.get((x, y)) {
                continue;
            }
            if layout.masks.anchor_mask.get((x, y)) {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::SandMaskMismatch,
                    message: format!("final_sand_mask overlaps anchor_mask at ({x},{y})"),
                });
            }
            if layout.masks.river_protection_band.get((x, y)) {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::SandMaskMismatch,
                    message: format!("final_sand_mask overlaps river_protection_band at ({x},{y})"),
                });
            }
        }
    }
}

// ── テスト ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen::generate_world_layout;
    use hw_core::constants::MAP_WIDTH;

    const GOLDEN_SEED_STANDARD: u64 = 42;

    #[test]
    fn test_golden_seeds_pass_lightweight_validate() {
        for seed in [GOLDEN_SEED_STANDARD] {
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
        let mut layout = generate_world_layout(GOLDEN_SEED_STANDARD);
        // Site の左上角を River に書き換える
        let min_x = layout.anchors.site.min_x;
        let min_y = layout.anchors.site.min_y;
        let idx = (min_y * MAP_WIDTH + min_x) as usize;
        layout.terrain_tiles[idx] = TerrainType::River;
        assert!(lightweight_validate(&layout).is_err());
    }
}
