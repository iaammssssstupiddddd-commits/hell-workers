use super::PlacementQueries;
use super::companion::parent_building_type;
use super::placement::{place_building_blueprint, try_place_bucket_storage_companion};
use crate::app_contexts::{CompanionPlacementKind, CompanionPlacementState};
use crate::assets::GameAssets;
use crate::world::map::{RIVER_Y_MIN, WorldMap};
use bevy::prelude::*;
use hw_ui::selection::{PlacementRejectReason, PlacementTileRejection, building_occupied_grids};

/// Handles the companion placement flow when `companion_state` is active.
/// Returns `true` if the flow consumed the click (caller should `return`).
pub(super) fn handle_companion_flow(
    companion_state: &mut CompanionPlacementState,
    commands: &mut Commands,
    world_map: &mut WorldMap,
    game_assets: &GameAssets,
    pq: &PlacementQueries<'_, '_, '_>,
    world_pos: Vec2,
    grid: (i32, i32),
) -> Option<Result<(), PlacementTileRejection>> {
    let active = companion_state.0.clone()?;

    if world_pos.distance(active.center) > active.radius {
        return Some(Err(PlacementTileRejection {
            grid,
            reason: PlacementRejectReason::TooFarFromParent,
        }));
    }

    match active.kind {
        CompanionPlacementKind::BucketStorage => {
            let parent_type = parent_building_type(active.parent_kind);
            let parent_occupied_grids =
                building_occupied_grids(parent_type, active.parent_anchor, RIVER_Y_MIN);

            let (parent_blueprint, _, _) = match place_building_blueprint(
                commands,
                world_map,
                game_assets,
                parent_type,
                active.parent_anchor,
                pq,
            ) {
                Ok(placed) => placed,
                Err(rejection) => {
                    warn!("COMPANION: parent blueprint validation failed");
                    return Some(Err(rejection));
                }
            };
            match try_place_bucket_storage_companion(
                commands,
                world_map,
                parent_blueprint,
                &parent_occupied_grids,
                grid,
            ) {
                Ok(()) => {
                    companion_state.0 = None;
                    Some(Ok(()))
                }
                Err(rejection) => {
                    // 親Blueprintの確定に成功したが companion が置けない場合は巻き戻す
                    for &(gx, gy) in &parent_occupied_grids {
                        world_map.clear_building_occupancy((gx, gy));
                    }
                    commands.entity(parent_blueprint).despawn();
                    Some(Err(rejection))
                }
            }
        }
    }
}
