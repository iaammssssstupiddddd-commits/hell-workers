//! タスクリストのオーケストレーション

use crate::interface::ui::components::{LeftPanelMode, TaskListBody};
use crate::interface::ui::theme::UiTheme;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::{Blueprint, BonePile, Designation, Priority, Rock, SandPile, Tree};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::TransportRequest;
use bevy::prelude::*;

use super::render;
use super::view_model::{self, TaskListState};

pub use super::interaction::{
    left_panel_tab_system, left_panel_visibility_system, task_list_click_system,
    task_list_visual_feedback_system,
};

pub fn task_list_update_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
    mode: Res<LeftPanelMode>,
    mut state: ResMut<TaskListState>,
    designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&Priority>,
        Option<&TaskWorkers>,
        Option<&Blueprint>,
        Option<&TransportRequest>,
        Option<&ResourceItem>,
        Option<&Tree>,
        Option<&Rock>,
        Option<&SandPile>,
        Option<&BonePile>,
    )>,
    body_query: Query<Entity, With<TaskListBody>>,
    children_query: Query<&Children>,
) {
    if *mode != LeftPanelMode::TaskList {
        return;
    }

    let snapshot = view_model::build_task_list_snapshot(&designations);

    if snapshot == state.last_snapshot {
        return;
    }
    state.last_snapshot = snapshot.clone();

    let Ok(body_entity) = body_query.single() else {
        return;
    };

    if let Ok(children) = children_query.get(body_entity) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    commands.entity(body_entity).with_children(|parent| {
        render::rebuild_task_list_ui(parent, &snapshot, &game_assets, &theme);
    });
}
