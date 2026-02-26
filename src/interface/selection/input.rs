use crate::entities::damned_soul::{DamnedSoul, Destination};
use crate::entities::familiar::Familiar;
use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::ui::UiInputState;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::prelude::*;

use super::hit_test::{
    hovered_entity_at_world_pos, hovered_task_area_border_entity, selectable_worker_at_world_pos,
};
use super::state::{HoveredEntity, SelectedEntity};

pub fn handle_mouse_input(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_souls: Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_task_areas: Query<(Entity, &TaskArea), With<Familiar>>,
    q_targets: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&crate::systems::jobs::Building>,
        ),
        Or<(
            With<crate::systems::jobs::Tree>,
            With<crate::systems::jobs::Rock>,
            With<crate::systems::logistics::ResourceItem>,
            With<crate::systems::jobs::Building>,
        )>,
    >,
    ui_input_state: Res<UiInputState>,
    mut selected_entity: ResMut<SelectedEntity>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut task_context: ResMut<TaskContext>,
    mut q_dest: Query<&mut Destination>,
) {
    // main.rsでrun_if(in_state(PlayMode::Normal))が設定されているため、
    // TaskModeのチェックは不要

    if ui_input_state.pointer_over_ui {
        return;
    }

    let Some(world_pos) = super::placement_common::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };

    if buttons.just_pressed(MouseButton::Left) {
        if let Some(familiar_entity) =
            hovered_task_area_border_entity(world_pos, selected_entity.0, &q_task_areas)
        {
            selected_entity.0 = Some(familiar_entity);
            task_context.0 = TaskMode::AreaSelection(None);
            next_play_mode.set(PlayMode::TaskDesignation);
            return;
        }

        selected_entity.0 = selectable_worker_at_world_pos(world_pos, &q_souls, &q_familiars);
    }

    if buttons.just_pressed(MouseButton::Right) {
        // 右クリック対象がエンティティ上なら、移動命令ではなくコンテキストメニューを優先する。
        if hovered_entity_at_world_pos(world_pos, &q_souls, &q_familiars, &q_targets).is_some() {
            return;
        }

        if let Some(selected) = selected_entity.0 {
            // 使い魔の場合のみ移動指示を出す（Soulは直接指示不可）
            if q_familiars.get(selected).is_ok() {
                if let Ok(mut dest) = q_dest.get_mut(selected) {
                    dest.0 = world_pos;
                }
            }
        }
    }
}

pub fn update_hover_entity(
    ui_input_state: Res<UiInputState>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_souls: Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_targets: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&crate::systems::jobs::Building>,
        ),
        Or<(
            With<crate::systems::jobs::Tree>,
            With<crate::systems::jobs::Rock>,
            With<crate::systems::logistics::ResourceItem>,
            With<crate::systems::jobs::Building>,
        )>,
    >,
    mut hovered_entity: ResMut<HoveredEntity>,
) {
    if ui_input_state.pointer_over_ui {
        hovered_entity.0 = None;
        return;
    }

    let Some(world_pos) = super::placement_common::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let found = hovered_entity_at_world_pos(world_pos, &q_souls, &q_familiars, &q_targets);

    if found != hovered_entity.0 {
        hovered_entity.0 = found;
    }
}
