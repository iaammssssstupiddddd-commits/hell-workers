use crate::systems::jobs::{Blueprint, BonePile, Designation, Priority, Rock, SandPile, Tree};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::TransportRequest;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::ecs::drain_removed;
use hw_core::relationships::TaskWorkers;
use hw_ui::components::LeftPanelMode;

pub use hw_ui::panels::task_list::TaskListDirty;

#[derive(SystemParam)]
pub struct TaskChangedDetectors<'w, 's> {
    q_designations: Query<'w, 's, (), Changed<Designation>>,
    q_added_designations: Query<'w, 's, (), Added<Designation>>,
    q_priority: Query<'w, 's, (), Changed<Priority>>,
    q_task_workers: Query<'w, 's, (), Changed<TaskWorkers>>,
    q_blueprints: Query<'w, 's, (), Changed<Blueprint>>,
    q_transport_requests: Query<'w, 's, (), Changed<TransportRequest>>,
    q_resource_items: Query<'w, 's, (), Changed<ResourceItem>>,
    q_trees: Query<'w, 's, (), Changed<Tree>>,
    q_rocks: Query<'w, 's, (), Changed<Rock>>,
    q_sand_piles: Query<'w, 's, (), Changed<SandPile>>,
    q_bone_piles: Query<'w, 's, (), Changed<BonePile>>,
}

pub fn detect_task_list_changed_components(
    mut dirty: ResMut<TaskListDirty>,
    mode: Res<LeftPanelMode>,
    detectors: TaskChangedDetectors,
) {
    let TaskChangedDetectors {
        q_designations,
        q_added_designations,
        q_priority,
        q_task_workers,
        q_blueprints,
        q_transport_requests,
        q_resource_items,
        q_trees,
        q_rocks,
        q_sand_piles,
        q_bone_piles,
    } = detectors;
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
        // パネル表示切替時はスナップショットも再構築する
        dirty.mark_all();
    }
}

#[derive(SystemParam)]
pub struct TaskRemovedDetectors<'w, 's> {
    removed_designations: RemovedComponents<'w, 's, Designation>,
    removed_priority: RemovedComponents<'w, 's, Priority>,
    removed_task_workers: RemovedComponents<'w, 's, TaskWorkers>,
    removed_blueprints: RemovedComponents<'w, 's, Blueprint>,
    removed_transport_requests: RemovedComponents<'w, 's, TransportRequest>,
    removed_resource_items: RemovedComponents<'w, 's, ResourceItem>,
    removed_trees: RemovedComponents<'w, 's, Tree>,
    removed_rocks: RemovedComponents<'w, 's, Rock>,
    removed_sand_piles: RemovedComponents<'w, 's, SandPile>,
    removed_bone_piles: RemovedComponents<'w, 's, BonePile>,
}

pub fn detect_task_list_removed_components(
    mut dirty: ResMut<TaskListDirty>,
    mut removed: TaskRemovedDetectors,
) {
    // 全 reader を1件だけ進めずに最後まで消費する。
    let mut removed_any = false;
    removed_any |= drain_removed(&mut removed.removed_designations);
    removed_any |= drain_removed(&mut removed.removed_priority);
    removed_any |= drain_removed(&mut removed.removed_task_workers);
    removed_any |= drain_removed(&mut removed.removed_blueprints);
    removed_any |= drain_removed(&mut removed.removed_transport_requests);
    removed_any |= drain_removed(&mut removed.removed_resource_items);
    removed_any |= drain_removed(&mut removed.removed_trees);
    removed_any |= drain_removed(&mut removed.removed_rocks);
    removed_any |= drain_removed(&mut removed.removed_sand_piles);
    removed_any |= drain_removed(&mut removed.removed_bone_piles);

    if removed_any {
        dirty.mark_all();
    }
}
