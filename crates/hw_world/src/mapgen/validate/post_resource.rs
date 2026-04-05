use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;

use std::collections::HashSet;

use crate::mapgen::resources::ResourceLayout;
use crate::mapgen::types::GeneratedWorldLayout;
use crate::pathfinding::{PathWorld, PathfindingContext, can_reach_target};
use crate::terrain::TerrainType;

use super::ValidationError;

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
