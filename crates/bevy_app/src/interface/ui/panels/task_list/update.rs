// タスクリストのオーケストレーション

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::{LeftPanelMode, TaskListBody};
use hw_ui::theme::UiTheme;

use super::{TaskListDirty, view_model::TaskListState};

#[derive(SystemParam)]
pub struct TaskListRenderState<'w> {
    game_assets: Res<'w, crate::assets::GameAssets>,
    theme: Res<'w, UiTheme>,
    mode: Res<'w, LeftPanelMode>,
    state: Res<'w, TaskListState>,
}

pub fn task_list_update_system(
    mut commands: Commands,
    render_state: TaskListRenderState,
    mut dirty: ResMut<TaskListDirty>,
    body_query: Query<Entity, With<TaskListBody>>,
    children_query: Query<&Children>,
) {
    if *render_state.mode != LeftPanelMode::TaskList {
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
            &render_state.state.snapshot,
            &*render_state.game_assets,
            &render_state.theme,
        );
    });
    dirty.clear_list();
}
