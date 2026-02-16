//! タスクリストのスナップショット生成

use crate::relationships::TaskWorkers;
use crate::systems::jobs::{
    Blueprint, BonePile, Designation, Priority, Rock, SandPile, Tree, WorkType,
};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::TransportRequest;
use bevy::prelude::*;
use std::collections::BTreeMap;

use super::presenter;

#[derive(Clone, PartialEq)]
pub struct TaskEntry {
    pub entity: Entity,
    pub description: String,
    pub priority: u32,
    pub worker_count: usize,
}

#[derive(Resource, Default)]
pub struct TaskListState {
    pub last_snapshot: Vec<(WorkType, Vec<TaskEntry>)>,
}

/// Designation クエリからスナップショットを構築
pub fn build_task_list_snapshot(
    designations: &Query<(
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
            blueprint,
            transport_req,
            resource_item,
            tree,
            rock,
            sand_pile,
            bone_pile,
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
