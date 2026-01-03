//! タスクキュー管理モジュール
//!
//! 使い魔ごとのタスクキューと未アサインタスクキューを管理します。

use crate::systems::jobs::{DesignationCreatedEvent, WorkType};
use bevy::prelude::*;
use std::collections::HashMap;

// ============================================================
// タスクキュー構造体
// ============================================================

/// タスクキュー - 保留中の仕事を管理
#[derive(Resource, Default)]
pub struct TaskQueue {
    pub by_familiar: HashMap<Entity, Vec<PendingTask>>,
}

/// 未アサインタスクキュー - 使い魔に割り当てられていないタスク
#[derive(Resource, Default)]
pub struct GlobalTaskQueue {
    pub unassigned: Vec<PendingTask>,
}

#[derive(Clone, Copy, Debug)]
pub struct PendingTask {
    pub entity: Entity,
    pub work_type: WorkType,
    pub priority: u32, // 0: Normal, 1: High, etc.
}

impl TaskQueue {
    pub fn add(&mut self, familiar: Entity, task: PendingTask) {
        let tasks = self.by_familiar.entry(familiar).or_default();
        tasks.push(task);
        // 優先度でソート (降順)
        tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn get_for_familiar(&self, familiar: Entity) -> Option<&Vec<PendingTask>> {
        self.by_familiar.get(&familiar)
    }

    pub fn remove(&mut self, familiar: Entity, task_entity: Entity) {
        if let Some(tasks) = self.by_familiar.get_mut(&familiar) {
            tasks.retain(|t| t.entity != task_entity);
        }
    }
}

impl GlobalTaskQueue {
    pub fn add(&mut self, task: PendingTask) {
        self.unassigned.push(task);
        self.unassigned.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn remove(&mut self, task_entity: Entity) {
        self.unassigned.retain(|t| t.entity != task_entity);
    }
}

// ============================================================
// キュー管理システム
// ============================================================

/// DesignationCreatedEventを受けてキューに追加するシステム
pub fn queue_management_system(
    mut queue: ResMut<TaskQueue>,
    mut global_queue: ResMut<GlobalTaskQueue>,
    mut ev_created: EventReader<DesignationCreatedEvent>,
) {
    for ev in ev_created.read() {
        let task = PendingTask {
            entity: ev.entity,
            work_type: ev.work_type,
            priority: ev.priority,
        };

        if let Some(issued_by) = ev.issued_by {
            queue.add(issued_by, task);
            if ev.priority > 0 {
                info!(
                    "QUEUE: High Priority Task added for Familiar {:?}",
                    issued_by
                );
            }
        } else {
            global_queue.add(task);
            info!("QUEUE: Unassigned Task added (entity: {:?})", ev.entity);
        }
    }
}
