use super::queries::{
    DesignationTargetQuery, FloorTileBlueprintQuery, UnassignedDesignationQuery,
    WallTileBlueprintQuery,
};
use super::{AreaEditHistory, AreaEditSession};
use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::command::{AreaSelectionIndicator, TaskArea, TaskMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::game_state::PlayMode;
use hw_ui::camera::{MainCamera, world_cursor_pos};
use hw_world::zones::Site;

mod drag;
mod press;
mod release;
mod transitions;

use drag::{ActiveDragCtx, handle_active_drag_input};
use press::handle_left_just_pressed_input;
use release::{ReleaseCtx, handle_left_just_released_input};

#[derive(SystemParam)]
pub struct AreaInputContext<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<'w, UiInputState>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
}

#[derive(SystemParam)]
pub struct AreaStateParams<'w> {
    selected: Res<'w, SelectedEntity>,
    task_context: ResMut<'w, TaskContext>,
    next_play_mode: ResMut<'w, NextState<PlayMode>>,
    area_edit_session: ResMut<'w, AreaEditSession>,
    area_edit_history: ResMut<'w, AreaEditHistory>,
}

#[derive(SystemParam)]
pub struct AreaEntityQueries<'w, 's> {
    q_familiars: Query<'w, 's, (&'static mut ActiveCommand, &'static mut Destination), With<Familiar>>,
    q_familiar_areas: Query<'w, 's, &'static TaskArea, With<Familiar>>,
    q_target_sets: ParamSet<'w, 's, (
        DesignationTargetQuery<'w, 's>,
        FloorTileBlueprintQuery<'w, 's>,
        WallTileBlueprintQuery<'w, 's>,
    )>,
    q_sites: Query<'w, 's, &'static Site>,
    q_aux: ParamSet<'w, 's, (
        UnassignedDesignationQuery<'w, 's>,
        Query<'w, 's, Entity, With<AreaSelectionIndicator>>,
    )>,
}

pub fn task_area_selection_system(
    input: AreaInputContext,
    mut state: AreaStateParams,
    mut queries: AreaEntityQueries,
    mut commands: Commands,
) {
    if !matches!(state.task_context.0, TaskMode::DreamPlanting(_)) {
        state.area_edit_session.dream_planting_preview_seed = None;
    }

    if input.ui_input_state.pointer_over_ui {
        return;
    }

    if state.task_context.0 == TaskMode::None {
        state.area_edit_session.active_drag = None;
        return;
    }

    if handle_active_drag_input(
        &mut ActiveDragCtx {
            buttons: &input.buttons,
            keyboard: &input.keyboard,
            task_context: &mut state.task_context,
            next_play_mode: &mut state.next_play_mode,
            area_edit_session: &mut state.area_edit_session,
            area_edit_history: &mut state.area_edit_history,
        },
        &input.q_window,
        &input.q_camera,
        &mut queries.q_familiars,
        &queries.q_sites,
        &queries.q_aux.p0(),
        &mut commands,
    ) {
        return;
    }

    if input.buttons.just_pressed(MouseButton::Left)
        && handle_left_just_pressed_input(
            &mut state.task_context,
            state.selected.0,
            &queries.q_familiar_areas,
            &input.q_window,
            &input.q_camera,
            &mut state.area_edit_session,
        )
    {
        return;
    }

    if !input.buttons.just_released(MouseButton::Left) {
        return;
    }

    let Some(world_pos) = world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };

    handle_left_just_released_input(
        &mut ReleaseCtx {
            task_context: &mut state.task_context,
            selected_entity: state.selected.0,
            world_pos,
            keyboard: &input.keyboard,
            next_play_mode: &mut state.next_play_mode,
            area_edit_session: &mut state.area_edit_session,
            area_edit_history: &mut state.area_edit_history,
        },
        &queries.q_familiar_areas,
        &queries.q_sites,
        &mut queries.q_familiars,
        &mut queries.q_target_sets,
        &mut queries.q_aux,
        &mut commands,
    );
}
