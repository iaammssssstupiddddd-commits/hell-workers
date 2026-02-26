use crate::assets::GameAssets;
use crate::game_state::{CompanionPlacementKind, CompanionPlacementState};
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::companion::parent_building_type;
use super::geometry::occupied_grids_for_building;
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
                occupied_grids_for_building(parent_type, active.parent_anchor);

            let Some((parent_blueprint, _, _)) = place_building_blueprint(
                commands,
                world_map,
                game_assets,
                parent_type,
                active.parent_anchor,
                q_buildings,
                q_blueprints_by_entity,
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
                    world_map.buildings.remove(&(gx, gy));
                    world_map.remove_obstacle(gx, gy);
                }
                commands.entity(parent_blueprint).despawn();
            }
        }
        CompanionPlacementKind::SandPile => {
            let parent_type = parent_building_type(active.parent_kind);
            let parent_occupied_grids =
                occupied_grids_for_building(parent_type, active.parent_anchor);
            if parent_occupied_grids.contains(&grid) {
                return true;
            }
            if place_building_blueprint(
                commands,
                world_map,
                game_assets,
                BuildingType::SandPile,
                grid,
                q_buildings,
                q_blueprints_by_entity,
            )
            .is_some()
            {
                if place_building_blueprint(
                    commands,
                    world_map,
                    game_assets,
                    parent_type,
                    active.parent_anchor,
                    q_buildings,
                    q_blueprints_by_entity,
                )
                .is_some()
                {
                    companion_state.0 = None;
                } else {
                    warn!("COMPANION: SandPile placed but failed to confirm parent blueprint");
                }
            }
        }
    }
    true
}
