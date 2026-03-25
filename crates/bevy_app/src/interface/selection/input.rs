use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::{DamnedSoul, Destination};
use crate::entities::familiar::Familiar;
use crate::interface::ui::UiInputState;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;
use hw_ui::selection::SelectionIntent;

use super::hit_test::{
    hovered_entity_at_world_pos, hovered_task_area_border_entity, selectable_worker_at_world_pos,
};
use super::state::{HoveredEntity, SelectedEntity};

type SelectionTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        Option<&'static crate::systems::jobs::Building>,
    ),
    Or<(
        With<crate::systems::jobs::Tree>,
        With<crate::systems::jobs::Rock>,
        With<crate::systems::logistics::ResourceItem>,
        With<crate::systems::jobs::Building>,
    )>,
>;

#[derive(SystemParam)]
pub struct SelectionInput<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    pub ui_input_state: Res<'w, UiInputState>,
}

#[derive(SystemParam)]
pub struct SelectionWorldQueries<'w, 's> {
    pub q_souls: Query<'w, 's, (Entity, &'static GlobalTransform), With<DamnedSoul>>,
    pub q_familiars: Query<'w, 's, (Entity, &'static GlobalTransform), With<Familiar>>,
    pub q_task_areas: Query<'w, 's, (Entity, &'static TaskArea), With<Familiar>>,
    pub q_targets: SelectionTargetQuery<'w, 's>,
}

/// Determines the SelectionIntent for a left-click at `world_pos`.
fn resolve_left_click_intent(
    world_pos: Vec2,
    current_selected: Option<Entity>,
    q_souls: &Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: &Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_task_areas: &Query<(Entity, &TaskArea), With<Familiar>>,
) -> SelectionIntent {
    if let Some(familiar) =
        hovered_task_area_border_entity(world_pos, current_selected, q_task_areas)
    {
        return SelectionIntent::StartAreaSelection { familiar };
    }

    match selectable_worker_at_world_pos(world_pos, q_souls, q_familiars) {
        Some(entity) => SelectionIntent::Select(entity),
        None => SelectionIntent::ClearSelection,
    }
}

/// Determines the SelectionIntent for a right-click at `world_pos`.
fn resolve_right_click_intent(
    world_pos: Vec2,
    current_selected: Option<Entity>,
    q_souls: &Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: &Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_targets: &SelectionTargetQuery,
) -> SelectionIntent {
    // 右クリック対象がエンティティ上なら移動命令ではなくコンテキストメニューを優先
    if hovered_entity_at_world_pos(world_pos, q_souls, q_familiars, q_targets).is_some() {
        return SelectionIntent::None;
    }

    if let Some(familiar) = current_selected
        && q_familiars.get(familiar).is_ok()
    {
        return SelectionIntent::MoveFamiliar {
            familiar,
            destination: world_pos,
        };
    }

    SelectionIntent::None
}

pub fn handle_mouse_input(
    input: SelectionInput,
    world_queries: SelectionWorldQueries,
    mut selected_entity: ResMut<SelectedEntity>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut task_context: ResMut<TaskContext>,
    mut q_dest: Query<&mut Destination>,
) {
    let SelectionInput {
        buttons,
        q_window,
        q_camera,
        ui_input_state,
    } = input;
    let SelectionWorldQueries {
        q_souls,
        q_familiars,
        q_task_areas,
        q_targets,
    } = world_queries;
    if ui_input_state.pointer_over_ui {
        return;
    }

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };

    if buttons.just_pressed(MouseButton::Left) {
        let intent = resolve_left_click_intent(
            world_pos,
            selected_entity.0,
            &q_souls,
            &q_familiars,
            &q_task_areas,
        );
        apply_selection_intent(
            intent,
            &mut selected_entity,
            &mut next_play_mode,
            &mut task_context,
            &mut q_dest,
        );
    }

    if buttons.just_pressed(MouseButton::Right) {
        let intent = resolve_right_click_intent(
            world_pos,
            selected_entity.0,
            &q_souls,
            &q_familiars,
            &q_targets,
        );
        apply_selection_intent(
            intent,
            &mut selected_entity,
            &mut next_play_mode,
            &mut task_context,
            &mut q_dest,
        );
    }
}

/// Applies a `SelectionIntent` to ECS state. This is the root-side adapter.
fn apply_selection_intent(
    intent: SelectionIntent,
    selected_entity: &mut SelectedEntity,
    next_play_mode: &mut NextState<PlayMode>,
    task_context: &mut TaskContext,
    q_dest: &mut Query<&mut Destination>,
) {
    match intent {
        SelectionIntent::Select(entity) => {
            selected_entity.0 = Some(entity);
        }
        SelectionIntent::ClearSelection => {
            selected_entity.0 = None;
        }
        SelectionIntent::StartAreaSelection { familiar } => {
            selected_entity.0 = Some(familiar);
            task_context.0 = TaskMode::AreaSelection(None);
            next_play_mode.set(PlayMode::TaskDesignation);
        }
        SelectionIntent::MoveFamiliar {
            familiar,
            destination,
        } => {
            if let Ok(mut dest) = q_dest.get_mut(familiar) {
                dest.0 = destination;
            }
        }
        SelectionIntent::None => {}
    }
}

pub fn update_hover_entity(
    ui_input_state: Res<UiInputState>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_souls: Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_targets: SelectionTargetQuery,
    mut hovered_entity: ResMut<HoveredEntity>,
) {
    if ui_input_state.pointer_over_ui {
        hovered_entity.0 = None;
        return;
    }

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let found = hovered_entity_at_world_pos(world_pos, &q_souls, &q_familiars, &q_targets);

    if found != hovered_entity.0 {
        hovered_entity.0 = found;
    }
}
