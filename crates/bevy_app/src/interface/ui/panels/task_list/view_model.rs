//! タスクリストのスナップショット生成

use super::dirty::TaskListDirty;
use crate::systems::jobs::{
    Blueprint, BonePile, Designation, Priority, Rock, SandPile, Tree, WorkType,
};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::TransportRequest;
use bevy::prelude::*;
use hw_core::relationships::TaskWorkers;
use std::collections::BTreeMap;

use super::presenter;

pub use hw_ui::panels::task_list::TaskEntry;

#[derive(Resource, Default)]
pub struct TaskListState {
    pub snapshot: Vec<(WorkType, Vec<TaskEntry>)>,
    pub summary_total: usize,
    pub summary_high: usize,
    initialized: bool,
}

type DesignationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Designation,
        Option<&'static Priority>,
        Option<&'static TaskWorkers>,
        Option<&'static Blueprint>,
        Option<&'static TransportRequest>,
        Option<&'static ResourceItem>,
        Option<&'static Tree>,
        Option<&'static Rock>,
        Option<&'static SandPile>,
        Option<&'static BonePile>,
    ),
>;

/// Designation クエリからスナップショットを構築
pub fn build_task_list_snapshot(
    designations: &DesignationQuery,
) -> Vec<(WorkType, Vec<TaskEntry>)> {
    let mut groups: BTreeMap<u8, (WorkType, Vec<TaskEntry>)> = BTreeMap::new();

    for (
        entity,
        _transform,
        designation,
        priority,
        workers,
        blueprint,
        transport_req,
        resource_item,
        tree,
        rock,
        sand_pile,
        bone_pile,
    ) in designations.iter()
    {
        let wt = designation.work_type;
        let key = wt as u8;

        let description = presenter::generate_task_description(
            wt,
            entity,
            presenter::TaskComponentRefs {
                blueprint,
                transport_req,
                resource_item,
                tree,
                rock,
                _sand_pile: sand_pile,
                bone_pile,
            },
        );

        let entry = TaskEntry {
            entity,
            description,
            priority: priority.map_or(0, |p| p.0),
            worker_count: workers.map_or(0, |w| w.iter().count()),
        };
        groups
            .entry(key)
            .or_insert_with(|| (wt, Vec::new()))
            .1
            .push(entry);
    }

    groups.into_values().collect()
}

pub fn build_task_summary(designations: &DesignationQuery) -> (usize, usize) {
    let mut total = 0usize;
    let mut high = 0usize;

    for (_, _, _, priority, _, _, _, _, _, _, _, _) in designations.iter() {
        total += 1;
        if priority.is_some_and(|p| p.0 > 0) {
            high += 1;
        }
    }

    (total, high)
}

pub fn update_task_list_state_system(
    designations: DesignationQuery,
    mut dirty: ResMut<TaskListDirty>,
    mut state: ResMut<TaskListState>,
) {
    if state.initialized && !dirty.state_dirty() {
        return;
    }

    let snapshot = build_task_list_snapshot(&designations);
    let (summary_total, summary_high) = build_task_summary(&designations);
    let list_changed = !state.initialized || snapshot != state.snapshot;
    let summary_changed = !state.initialized
        || summary_total != state.summary_total
        || summary_high != state.summary_high;

    state.snapshot = snapshot;
    state.summary_total = summary_total;
    state.summary_high = summary_high;
    let was_initialized = state.initialized;
    state.initialized = true;
    dirty.clear_state();

    if !was_initialized || list_changed {
        dirty.mark_list();
    } else {
        dirty.clear_list();
    }
    if !was_initialized || summary_changed {
        dirty.mark_summary();
    } else {
        dirty.clear_summary();
    }
}
