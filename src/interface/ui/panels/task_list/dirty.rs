use hw_ui::components::LeftPanelMode;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::{Blueprint, BonePile, Designation, Priority, Rock, SandPile, Tree};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::TransportRequest;
use bevy::prelude::*;

pub use hw_ui::panels::task_list::TaskListDirty;

pub fn detect_task_list_changed_components(
    mut dirty: ResMut<TaskListDirty>,
    mode: Res<LeftPanelMode>,
    q_designations: Query<(), Changed<Designation>>,
    q_added_designations: Query<(), Added<Designation>>,
    q_priority: Query<(), Changed<Priority>>,
    q_task_workers: Query<(), Changed<TaskWorkers>>,
    q_blueprints: Query<(), Changed<Blueprint>>,
    q_transport_requests: Query<(), Changed<TransportRequest>>,
    q_resource_items: Query<(), Changed<ResourceItem>>,
    q_trees: Query<(), Changed<Tree>>,
    q_rocks: Query<(), Changed<Rock>>,
    q_sand_piles: Query<(), Changed<SandPile>>,
    q_bone_piles: Query<(), Changed<BonePile>>,
) {
    let task_data_changed = !q_designations.is_empty()
        || !q_added_designations.is_empty()
        || !q_priority.is_empty()
        || !q_task_workers.is_empty()
        || !q_blueprints.is_empty()
        || !q_transport_requests.is_empty()
        || !q_resource_items.is_empty()
        || !q_trees.is_empty()
        || !q_rocks.is_empty()
        || !q_sand_piles.is_empty()
        || !q_bone_piles.is_empty();

    if task_data_changed {
        dirty.mark_all();
    } else if mode.is_changed() && *mode == LeftPanelMode::TaskList {
        dirty.mark_list();
    }
}

pub fn detect_task_list_removed_components(
    mut dirty: ResMut<TaskListDirty>,
    mut removed_designations: RemovedComponents<Designation>,
    mut removed_priority: RemovedComponents<Priority>,
    mut removed_task_workers: RemovedComponents<TaskWorkers>,
    mut removed_blueprints: RemovedComponents<Blueprint>,
    mut removed_transport_requests: RemovedComponents<TransportRequest>,
    mut removed_resource_items: RemovedComponents<ResourceItem>,
    mut removed_trees: RemovedComponents<Tree>,
    mut removed_rocks: RemovedComponents<Rock>,
    mut removed_sand_piles: RemovedComponents<SandPile>,
    mut removed_bone_piles: RemovedComponents<BonePile>,
) {
    let removed_any = removed_designations.read().next().is_some()
        || removed_priority.read().next().is_some()
        || removed_task_workers.read().next().is_some()
        || removed_blueprints.read().next().is_some()
        || removed_transport_requests.read().next().is_some()
        || removed_resource_items.read().next().is_some()
        || removed_trees.read().next().is_some()
        || removed_rocks.read().next().is_some()
        || removed_sand_piles.read().next().is_some()
        || removed_bone_piles.read().next().is_some();

    if removed_any {
        dirty.mark_all();
    }
}
