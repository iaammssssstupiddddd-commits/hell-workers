use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;

use super::click_handlers::{clear_move_states, handle_companion_click, handle_initial_click};
use super::context::{BuildMoveInput, BuildMoveQueries, BuildMoveState, MoveOpCtx, MoveStateCtx};

pub fn building_move_system(
    input: BuildMoveInput,
    mut state: BuildMoveState,
    mut queries: BuildMoveQueries,
    mut world_map: WorldMapWrite,
    game_assets: Res<crate::assets::GameAssets>,
    mut commands: Commands,
) {
    if input.ui_input_state.pointer_over_ui {
        return;
    }
    if input.buttons.just_pressed(MouseButton::Right) {
        clear_move_states(
            &mut state.move_context,
            &mut state.move_placement_state,
            &mut state.companion_state,
        );
        state
            .next_play_mode
            .set(hw_core::game_state::PlayMode::Normal);
        return;
    }
    if !input.buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };
    let destination_grid = WorldMap::world_to_grid(world_pos);
    let Some(target_entity) = state.move_context.0 else {
        return;
    };

    let Ok((_, building, transform)) = queries.q_buildings.get(target_entity) else {
        clear_move_states(
            &mut state.move_context,
            &mut state.move_placement_state,
            &mut state.companion_state,
        );
        state
            .next_play_mode
            .set(hw_core::game_state::PlayMode::Normal);
        return;
    };
    use crate::systems::jobs::BuildingType;
    if !matches!(building.kind, BuildingType::Tank | BuildingType::MudMixer) {
        clear_move_states(
            &mut state.move_context,
            &mut state.move_placement_state,
            &mut state.companion_state,
        );
        state
            .next_play_mode
            .set(hw_core::game_state::PlayMode::Normal);
        return;
    }

    let mut op = MoveOpCtx {
        commands: &mut commands,
        world_map: &mut world_map,
        q_transport_requests: &queries.q_transport_requests,
        q_souls: &mut queries.q_souls,
        task_queries: &mut queries.task_queries,
        game_assets: &game_assets,
    };
    let mut st = MoveStateCtx {
        companion_state: &mut state.companion_state,
        move_placement_state: &mut state.move_placement_state,
        move_context: &mut state.move_context,
        next_play_mode: &mut state.next_play_mode,
    };

    if st.companion_state.0.is_some() {
        handle_companion_click(
            &mut op,
            &mut st,
            destination_grid,
            target_entity,
            building,
            transform,
            &queries.q_bucket_storages,
        );
        return;
    }

    handle_initial_click(
        &mut op,
        &mut st,
        destination_grid,
        target_entity,
        building,
        transform,
    );
}
