use crate::app_contexts::{
    CompanionParentKind, CompanionPlacement, CompanionPlacementKind, PendingMovePlacement,
};
use crate::systems::jobs::BuildingType;
use crate::world::map::{WorldMapRef, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_ui::selection::{
    PlacementTileRejection, move_anchor_grid, move_occupied_grids, move_spawn_pos,
    validate_moved_building_placement,
};

use super::context::{COMPANION_PLACEMENT_RADIUS_TILES, MoveOpCtx, MoveStateCtx};
use super::finalization::finalize_move_request;
use super::placement::validate_tank_companion_for_move;

/// Companion（BucketStorage）配置確認ステップのクリック処理。
pub(super) fn handle_companion_click(
    op: &mut MoveOpCtx<'_, '_, '_, '_, '_, '_>,
    st: &mut MoveStateCtx<'_>,
    destination_grid: (i32, i32),
    target_entity: Entity,
    building: &crate::systems::jobs::Building,
    transform: &Transform,
    q_bucket_storages: &Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
) -> Result<(), PlacementTileRejection> {
    let Some(active_companion) = st.companion_state.0.clone() else {
        return Ok(());
    };
    if active_companion.kind != CompanionPlacementKind::BucketStorage
        || active_companion.parent_kind != CompanionParentKind::Tank
    {
        st.companion_state.0 = None;
        st.move_placement_state.0 = None;
        return Ok(());
    }
    let Some(pending) = st.move_placement_state.0 else {
        st.companion_state.0 = None;
        return Ok(());
    };
    if pending.building != target_entity {
        st.companion_state.0 = None;
        st.move_placement_state.0 = None;
        return Ok(());
    }
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, pending.destination_grid);
    let parent_validation = validate_moved_building_placement(
        &WorldMapRef(op.world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    );
    if !parent_validation.can_place {
        return Err(parent_validation
            .rejection(pending.destination_grid)
            .expect("rejected moved building must carry a reason"));
    }
    let companion_validation = validate_tank_companion_for_move(
        op.world_map,
        target_entity,
        pending.destination_grid,
        destination_grid,
        &old_occupied,
        q_bucket_storages,
    );
    if !companion_validation.can_place {
        return Err(companion_validation
            .rejection(destination_grid)
            .expect("rejected moved companion must carry a reason"));
    }
    finalize_move_request(
        op,
        target_entity,
        building,
        transform,
        pending.destination_grid,
        Some(destination_grid),
    );
    clear_move_states(st.move_context, st.move_placement_state, st.companion_state);
    st.next_play_mode.set(hw_core::game_state::PlayMode::Normal);
    Ok(())
}

/// 初回クリック時の配置検証・移動確定処理。
/// Tank は companion 配置フローへ移行、MudMixer は即確定。
pub(super) fn handle_initial_click(
    op: &mut MoveOpCtx<'_, '_, '_, '_, '_, '_>,
    st: &mut MoveStateCtx<'_>,
    destination_grid: (i32, i32),
    target_entity: Entity,
    building: &crate::systems::jobs::Building,
    transform: &Transform,
) -> Result<(), PlacementTileRejection> {
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    let validation = validate_moved_building_placement(
        &WorldMapRef(op.world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    );
    if !validation.can_place {
        return Err(validation
            .rejection(destination_grid)
            .expect("rejected moved building must carry a reason"));
    }
    if building.kind == BuildingType::Tank {
        let center = move_spawn_pos(BuildingType::Tank, destination_grid);
        st.move_placement_state.0 = Some(PendingMovePlacement {
            building: target_entity,
            destination_grid,
        });
        st.companion_state.0 = Some(CompanionPlacement {
            parent_kind: CompanionParentKind::Tank,
            parent_anchor: destination_grid,
            kind: CompanionPlacementKind::BucketStorage,
            center,
            radius: TILE_SIZE * COMPANION_PLACEMENT_RADIUS_TILES,
            required: true,
        });
        return Ok(());
    }
    finalize_move_request(
        op,
        target_entity,
        building,
        transform,
        destination_grid,
        None,
    );
    clear_move_states(st.move_context, st.move_placement_state, st.companion_state);
    st.next_play_mode.set(hw_core::game_state::PlayMode::Normal);
    Ok(())
}

pub(crate) fn clear_move_states(
    move_context: &mut crate::app_contexts::MoveContext,
    move_placement_state: &mut crate::app_contexts::MovePlacementState,
    companion_state: &mut crate::app_contexts::CompanionPlacementState,
) {
    move_context.0 = None;
    move_placement_state.0 = None;
    companion_state.0 = None;
}

fn _worldmapwrite_deref_check(_: &WorldMapWrite<'_>) {}
