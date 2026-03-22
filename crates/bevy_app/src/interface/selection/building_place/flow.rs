use crate::app_contexts::{CompanionPlacementKind, CompanionPlacementState};
use crate::assets::GameAssets;
use crate::systems::jobs::{Blueprint, Building};
use hw_world::zones::{Site, Yard};
use crate::world::map::{RIVER_Y_MIN, WorldMap};
use bevy::prelude::*;
use hw_ui::selection::building_occupied_grids;

use super::companion::parent_building_type;
use super::placement::{place_building_blueprint, try_place_bucket_storage_companion};

/// Handles the companion placement flow when `companion_state` is active.
/// Returns `true` if the flow consumed the click (caller should `return`).
pub(super) fn handle_companion_flow(
    companion_state: &mut CompanionPlacementState,
    commands: &mut Commands,
    world_map: &mut WorldMap,
    game_assets: &GameAssets,
    q_buildings: &Query<&Building>,
    q_blueprints_by_entity: &Query<&Blueprint>,
    q_sites: &Query<&Site>,
    q_yards: &Query<&Yard>,
    world_pos: Vec2,
    grid: (i32, i32),
) -> bool {
    let Some(active) = companion_state.0.clone() else {
        return false;
    };

    if world_pos.distance(active.center) > active.radius {
        return true;
    }

    match active.kind {
        CompanionPlacementKind::BucketStorage => {
            let parent_type = parent_building_type(active.parent_kind);
            let parent_occupied_grids =
                building_occupied_grids(parent_type, active.parent_anchor, RIVER_Y_MIN);

            let Some((parent_blueprint, _, _)) = place_building_blueprint(
                commands,
                world_map,
                game_assets,
                parent_type,
                active.parent_anchor,
                q_buildings,
                q_blueprints_by_entity,
                q_sites,
                q_yards,
            ) else {
                warn!(
                    "COMPANION: failed to confirm parent blueprint before bucket storage placement"
                );
                return true;
            };
            if try_place_bucket_storage_companion(
                commands,
                world_map,
                parent_blueprint,
                &parent_occupied_grids,
                grid,
            ) {
                companion_state.0 = None;
            } else {
                // 親Blueprintの確定に成功したが companion が置けない場合は巻き戻す
                for &(gx, gy) in &parent_occupied_grids {
                    world_map.clear_building_occupancy((gx, gy));
                }
                commands.entity(parent_blueprint).despawn();
            }
        }
    }
    true
}
