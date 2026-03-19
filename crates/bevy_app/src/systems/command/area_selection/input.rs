use super::queries::DesignationTargetQuery;
use super::state::{AreaEditHistory, AreaEditSession};
use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::command::{AreaSelectionIndicator, TaskArea, TaskMode};
use crate::systems::jobs::Designation;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::wall_construction::WallTileBlueprint;
use crate::systems::world::zones::Site;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::game_state::PlayMode;
use hw_ui::camera::{MainCamera, world_cursor_pos};

mod drag;
mod press;
mod release;
mod transitions;

use drag::handle_active_drag_input;
use press::handle_left_just_pressed_input;
use release::handle_left_just_released_input;

pub fn task_area_selection_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_familiar_areas: Query<&TaskArea, With<Familiar>>,
    mut q_target_sets: ParamSet<(
        DesignationTargetQuery<'_, '_>,
        Query<(Entity, &Transform, &FloorTileBlueprint)>,
        Query<(Entity, &Transform, &WallTileBlueprint)>,
    )>,
    q_sites: Query<&Site>,
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut q_aux: ParamSet<(
        Query<(Entity, &Transform, &Designation), Without<hw_core::relationships::ManagedBy>>,
        Query<Entity, With<AreaSelectionIndicator>>,
    )>,
    mut area_edit_session: ResMut<AreaEditSession>,
    mut area_edit_history: ResMut<AreaEditHistory>,
) {
    if !matches!(task_context.0, TaskMode::DreamPlanting(_)) {
        area_edit_session.dream_planting_preview_seed = None;
    }

    if ui_input_state.pointer_over_ui {
        return;
    }

    if task_context.0 == TaskMode::None {
        area_edit_session.active_drag = None;
        return;
    }

    if handle_active_drag_input(
        &buttons,
        &q_window,
        &q_camera,
        &keyboard,
        &mut task_context,
        &mut next_play_mode,
        &mut q_familiars,
        &q_sites,
        &q_aux.p0(),
        &mut commands,
        &mut area_edit_session,
        &mut area_edit_history,
    ) {
        return;
    }

    if buttons.just_pressed(MouseButton::Left)
        && handle_left_just_pressed_input(
            &mut task_context,
            selected.0,
            &q_familiar_areas,
            &q_window,
            &q_camera,
            &mut area_edit_session,
        )
    {
        return;
    }

    if !buttons.just_released(MouseButton::Left) {
        return;
    }

    let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) else {
        return;
    };

    handle_left_just_released_input(
        &mut task_context,
        selected.0,
        world_pos,
        &q_familiar_areas,
        &q_sites,
        &mut q_familiars,
        &mut q_target_sets,
        &mut q_aux,
        &keyboard,
        &mut next_play_mode,
        &mut commands,
        &mut area_edit_session,
        &mut area_edit_history,
    );
}

