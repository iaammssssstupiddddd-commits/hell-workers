// タスクリストのオーケストレーション

use crate::interface::ui::components::{LeftPanelMode, TaskListBody};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

use super::{TaskListDirty, view_model::TaskListState};

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
        hw_ui::panels::task_list::rebuild_task_list_ui(
            parent,
            &state.snapshot,
            &*game_assets,
            &theme,
        );
    });
    dirty.clear_list();
}
