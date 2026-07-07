use bevy::prelude::*;
use hw_core::events::{ResourceReservationOp, ResourceReservationRequest};
use hw_core::relationships::WorkingOn;
use hw_core::soul::{DamnedSoul, Destination, Path, StressBreakdown};
use hw_core::visual::SoulTaskHandles;
use hw_logistics::types::Inventory;
use hw_world::{PathfindingContext, WorldMap};

use hw_jobs::AssignedTask;

use super::queries::TaskQueries;

/// タスク終了の種別（M2: 完了イベント誤発火防止）
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskEndDisposition {
    #[default]
    Running,
    Completed,
    AbortedRetryable,
    AbortedClosed,
}

/// フレーム内不変の実行環境（M1: ハンドラ引数から集約）
pub struct TaskExecEnv<'a> {
    pub soul_handles: &'a SoulTaskHandles,
    pub time: &'a Time,
    pub world_map: &'a WorldMap,
    pub breakdown: Option<&'a StressBreakdown>,
}

/// タスク実行の基本コンテキスト
pub struct TaskExecutionContext<'a, 'w, 's> {
    pub soul_entity: Entity,
    pub soul_transform: &'a Transform,
    pub soul: &'a mut DamnedSoul,
    pub task: &'a mut AssignedTask,
    pub dest: &'a mut Destination,
    pub path: &'a mut Path,
    pub inventory: &'a mut Inventory,
    pub pf_context: &'a mut PathfindingContext,
    pub queries: &'a mut TaskQueries<'w, 's>,
    pub env: TaskExecEnv<'a>,
    pub end_disposition: TaskEndDisposition,
}

impl<'a, 'w, 's> TaskExecutionContext<'a, 'w, 's> {
    /// 魂の位置を取得
    pub fn soul_pos(&self) -> Vec2 {
        self.soul_transform.translation.truncate()
    }

    /// リソース予約更新の要求を追加
    pub fn queue_reservation(&mut self, op: ResourceReservationOp) {
        self.queries
            .reservation
            .reservation_writer
            .write(ResourceReservationRequest { op });
    }

    /// 正常完了。成果物生成・需要消費・construction state 遷移は呼び出し元が済ませた後に呼ぶ。
    pub fn complete_task(&mut self, commands: &mut Commands, reason: &str) {
        debug!(
            "complete_task: soul {:?} - {}",
            self.soul_entity, reason
        );
        crate::soul_ai::execute::task_execution::common::clear_task_and_path(
            self.task,
            self.path,
        );
        commands.entity(self.soul_entity).remove::<WorkingOn>();
        self.end_disposition = TaskEndDisposition::Completed;
    }

    /// 再アサイン可能な中断。Designation / TransportRequest / construction state は残す。
    pub fn abort_retryable(&mut self, commands: &mut Commands, reason: &str) {
        debug!(
            "abort_retryable: soul {:?} - {}",
            self.soul_entity, reason
        );
        self.abort_with_disposition(commands, TaskEndDisposition::AbortedRetryable);
    }

    /// 対象消滅・designation 削除など、タスク本体が再試行不能な中断。
    pub fn abort_closed(&mut self, commands: &mut Commands, reason: &str) {
        debug!(
            "abort_closed: soul {:?} - {}",
            self.soul_entity, reason
        );
        self.abort_with_disposition(commands, TaskEndDisposition::AbortedClosed);
    }

    fn abort_with_disposition(
        &mut self,
        commands: &mut Commands,
        disposition: TaskEndDisposition,
    ) {
        use crate::soul_ai::helpers::work::{cleanup_task_assignment, SoulDropCtx};

        let soul_entity = self.soul_entity;
        let drop_pos = self.soul_pos();
        let world_map = self.env.world_map;

        cleanup_task_assignment(
            commands,
            SoulDropCtx {
                soul_entity,
                drop_pos,
                inventory: Some(self.inventory),
                dropped_item_res: None,
            },
            self.task,
            self.path,
            self.queries,
            world_map,
            false,
        );
        commands.entity(soul_entity).remove::<WorkingOn>();
        self.end_disposition = disposition;
    }

    /// 専用 cancel が予約・インベントリ cleanup 済みのとき、Soul 側割り当てだけ解除する。
    pub fn clear_soul_assignment(
        &mut self,
        commands: &mut Commands,
        disposition: TaskEndDisposition,
    ) {
        debug!(
            "clear_soul_assignment: soul {:?} disposition {:?}",
            self.soul_entity, disposition
        );
        crate::soul_ai::execute::task_execution::common::clear_task_and_path(
            self.task,
            self.path,
        );
        commands.entity(self.soul_entity).remove::<WorkingOn>();
        self.end_disposition = disposition;
    }
}
