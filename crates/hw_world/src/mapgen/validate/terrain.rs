use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;

use crate::mapgen::types::{GeneratedWorldLayout, ResourceSpawnCandidates};
use crate::pathfinding::{PathWorld, PathfindingContext, can_reach_target};
use crate::terrain::TerrainType;

use super::ValidationError;

// ── ValidatorPathWorld（内部ヘルパー） ────────────────────────────────────────

/// validate 内部専用。`terrain_tiles` スライスのみで PathWorld を実現する。
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
