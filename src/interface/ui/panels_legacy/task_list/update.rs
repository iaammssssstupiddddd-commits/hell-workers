// タスクリストのオーケストレーション

use crate::interface::ui::components::{LeftPanelMode, TaskListBody};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

use super::render;
use super::{
    TaskListDirty,
    view_model::{TaskListState},
};

pub use super::interaction::{
    left_panel_tab_system, left_panel_visibility_system, task_list_click_system,
    task_list_visual_feedback_system,
};

pub fn task_list_update_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
    mode: Res<LeftPanelMode>,
    mut dirty: ResMut<TaskListDirty>,
    state: Res<TaskListState>,
    body_query: Query<Entity, With<TaskListBody>>,
    children_query: Query<&Children>,
) {
    if *mode != LeftPanelMode::TaskList {
        return;
    }

    if !dirty.list_dirty() {
        return;
    }

    let Ok(body_entity) = body_query.single() else {
        return;
    };

    if let Ok(children) = children_query.get(body_entity) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    commands.entity(body_entity).with_children(|parent| {
        render::rebuild_task_list_ui(parent, &state.snapshot, &game_assets, &theme);
    });
    dirty.clear_list();
}
