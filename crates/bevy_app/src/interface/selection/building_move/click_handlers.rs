use crate::app_contexts::{
    CompanionParentKind, CompanionPlacement, CompanionPlacementKind, PendingMovePlacement,
};
use crate::systems::jobs::BuildingType;
use crate::world::map::{WorldMapRef, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_ui::selection::{
    can_place_moved_building, move_anchor_grid, move_occupied_grids, move_spawn_pos,
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
) {
    let Some(active_companion) = st.companion_state.0.clone() else {
        return;
    };
    if active_companion.kind != CompanionPlacementKind::BucketStorage
        || active_companion.parent_kind != CompanionParentKind::Tank
    {
        st.companion_state.0 = None;
        st.move_placement_state.0 = None;
        return;
    }
    let Some(pending) = st.move_placement_state.0 else {
        st.companion_state.0 = None;
        return;
    };
    if pending.building != target_entity {
        st.companion_state.0 = None;
        st.move_placement_state.0 = None;
        return;
    }
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, pending.destination_grid);
    if !can_place_moved_building(
        &WorldMapRef(op.world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    ) {
        return;
    }
    if !validate_tank_companion_for_move(
        op.world_map,
        target_entity,
        pending.destination_grid,
        destination_grid,
        &old_occupied,
        q_bucket_storages,
    )
    .can_place
    {
        return;
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
) {
    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    if !can_place_moved_building(
        &WorldMapRef(op.world_map),
        target_entity,
        &old_occupied,
        &destination_occupied,
    ) {
        return;
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
        return;
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
}

pub(super) fn clear_move_states(
    move_context: &mut crate::app_contexts::MoveContext,
    move_placement_state: &mut crate::app_contexts::MovePlacementState,
    companion_state: &mut crate::app_contexts::CompanionPlacementState,
) {
    move_context.0 = None;
    move_placement_state.0 = None;
    companion_state.0 = None;
}

fn _worldmapwrite_deref_check(_: &WorldMapWrite<'_>) {}
