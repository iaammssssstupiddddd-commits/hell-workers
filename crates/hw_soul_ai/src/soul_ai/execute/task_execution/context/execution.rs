use bevy::prelude::*;
use hw_core::events::{ResourceReservationOp, ResourceReservationRequest};
use hw_core::relationships::WorkingOn;
use hw_core::soul::{DamnedSoul, Destination, Path, StressBreakdown};
use hw_core::visual::SoulTaskHandles;
use hw_logistics::types::Inventory;
use hw_world::{PathfindingContext, RuntimePathSearchBudget, WorldMap};

use hw_jobs::{ActiveTaskIdentity, AssignedTask, WorkType, lifecycle};

use super::queries::TaskQueries;
use crate::soul_ai::execute::task_execution::path_cache::TaskPathSearchProgress;

/// タスク終了の種別（M2: 完了イベント誤発火防止）
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum TaskEndDisposition {
    #[default]
    Running,
    Completed,
    AbortedRetryable,
    AbortedClosed,
}

/// タスク handler がこの frame に terminal 処理を実行したかを表す。
#[must_use = "return TaskHandlerControl from the task handler so terminal branches cannot fall through"]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskHandlerControl {
    Continue,
    Ended,
    AlreadyEnded,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct AlreadyEnded {
    existing: TaskEndDisposition,
}

#[derive(Default)]
pub(crate) struct TaskEndState {
    disposition: TaskEndDisposition,
}

impl TaskEndState {
    fn try_begin_end(&mut self, next: TaskEndDisposition) -> Result<(), AlreadyEnded> {
        if self.disposition != TaskEndDisposition::Running {
            return Err(AlreadyEnded {
                existing: self.disposition,
            });
        }

        self.disposition = next;
        Ok(())
    }

    fn is_completed(&self) -> bool {
        self.disposition == TaskEndDisposition::Completed
    }
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
    /// `Mut` を所有して渡すことで、handler が実際に書き込むまで Changed を立てない。
    ///
    /// active task の早期deferはこの5コンポーネントを読むだけである。ここで
    /// `&mut T` に再借用すると、handlerに入る前に全コンポーネントが Changed になり、
    /// downstream の Changed query を毎frame起動してしまう。
    pub soul: Mut<'a, DamnedSoul>,
    pub task: Mut<'a, AssignedTask>,
    pub dest: Mut<'a, Destination>,
    pub path: Mut<'a, Path>,
    pub inventory: Mut<'a, Inventory>,
    pub(crate) identity: Mut<'a, ActiveTaskIdentity>,
    pub pf_context: &'a mut PathfindingContext,
    pub path_budget: &'a mut RuntimePathSearchBudget,
    pub(crate) path_search_progress: &'a mut TaskPathSearchProgress,
    pub queries: &'a mut TaskQueries<'w, 's>,
    pub env: TaskExecEnv<'a>,
    pub(crate) end_state: TaskEndState,
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

    /// 現在 segment の identity を返す。
    pub fn task_identity(&self) -> ActiveTaskIdentity {
        *self.identity
    }

    /// 同じ assignment 内で current target と work type を更新する。
    pub fn transition_task_identity(
        &mut self,
        current_target_entity: Entity,
        current_work_type: WorkType,
    ) {
        self.identity
            .transition_to(current_target_entity, current_work_type);
    }

    /// 成果確定後の Done phase が次 frame で terminal 処理を行うまで、
    /// `WorkingOn` が存在しないことを identity に記録する。
    pub fn detach_task_identity(&mut self) {
        self.identity.detach_from_working_on();
    }

    /// `OnTaskCompleted` を発行すべき正常終了かを返す。
    pub fn is_completed(&self) -> bool {
        self.end_state.is_completed()
    }

    fn try_begin_end(&mut self, disposition: TaskEndDisposition) -> Result<(), AlreadyEnded> {
        self.end_state.try_begin_end(disposition)
    }

    /// 現在の phase で保持している予約だけを解放する。
    ///
    /// 専用 cleanup 経路では task payload を消す前にここを通し、手動の
    /// `Release*` と lifecycle 契約が二重管理にならないようにする。
    fn release_active_task_reservations(&mut self) {
        let release_ops =
            lifecycle::collect_release_reservation_ops(&self.task, |item, fallback| {
                self.queries
                    .reservation
                    .resources
                    .get(item)
                    .ok()
                    .map(|resource| resource.0)
                    .unwrap_or(fallback)
            });

        for op in release_ops {
            self.queue_reservation(op);
        }
    }

    fn reject_duplicate_end(
        &self,
        reason: &str,
        already_ended: AlreadyEnded,
    ) -> TaskHandlerControl {
        error!(
            "TASK_EXEC: Soul {:?} ignored duplicate terminal transition ({reason}); existing disposition: {:?}",
            self.soul_entity, already_ended.existing
        );
        TaskHandlerControl::AlreadyEnded
    }

    /// 正常完了。成果物生成・需要消費・construction state 遷移は呼び出し元が済ませた後に呼ぶ。
    pub fn complete_task(&mut self, commands: &mut Commands, reason: &str) -> TaskHandlerControl {
        if let Err(already_ended) = self.try_begin_end(TaskEndDisposition::Completed) {
            return self.reject_duplicate_end(reason, already_ended);
        }
        debug!("complete_task: soul {:?} - {}", self.soul_entity, reason);
        crate::soul_ai::execute::task_execution::common::clear_task_and_path(
            &mut self.task,
            &mut self.path,
        );
        self.path_search_progress.clear_entity(self.soul_entity);
        commands
            .entity(self.soul_entity)
            .remove::<(WorkingOn, ActiveTaskIdentity)>();
        TaskHandlerControl::Ended
    }

    /// 再アサイン可能な中断。Designation / TransportRequest / construction state は残す。
    pub fn abort_retryable(&mut self, commands: &mut Commands, reason: &str) -> TaskHandlerControl {
        debug!("abort_retryable: soul {:?} - {}", self.soul_entity, reason);
        self.abort_with_disposition(commands, TaskEndDisposition::AbortedRetryable, reason)
    }

    /// 対象消滅・designation 削除など、タスク本体が再試行不能な中断。
    pub fn abort_closed(&mut self, commands: &mut Commands, reason: &str) -> TaskHandlerControl {
        debug!("abort_closed: soul {:?} - {}", self.soul_entity, reason);
        self.abort_with_disposition(commands, TaskEndDisposition::AbortedClosed, reason)
    }

    fn abort_with_disposition(
        &mut self,
        commands: &mut Commands,
        disposition: TaskEndDisposition,
        reason: &str,
    ) -> TaskHandlerControl {
        if let Err(already_ended) = self.try_begin_end(disposition) {
            return self.reject_duplicate_end(reason, already_ended);
        }

        use crate::soul_ai::helpers::work::{SoulDropCtx, cleanup_task_assignment};

        let soul_entity = self.soul_entity;
        let drop_pos = self.soul_pos();
        let world_map = self.env.world_map;

        cleanup_task_assignment(
            commands,
            SoulDropCtx {
                soul_entity,
                drop_pos,
                inventory: Some(&mut *self.inventory),
                dropped_item_res: None,
            },
            &mut self.task,
            &mut self.path,
            self.queries,
            world_map,
            false,
        );
        self.path_search_progress.clear_entity(soul_entity);
        commands.entity(soul_entity).remove::<WorkingOn>();
        TaskHandlerControl::Ended
    }

    /// 専用 cleanup が予約・インベントリ処理まで完了した後に、正常終了だけを確定する。
    pub fn complete_after_custom_cleanup(
        &mut self,
        commands: &mut Commands,
        reason: &str,
    ) -> TaskHandlerControl {
        self.end_with_assignment_cleanup(commands, TaskEndDisposition::Completed, reason)
    }

    /// 専用の物理 cleanup 後に retryable abort を確定する。
    ///
    /// 予約解放はここで lifecycle の現在 phase から一度だけ行う。呼び出し元は
    /// この task に対応する `Release*` を個別に発行してはならない。
    pub fn abort_retryable_after_custom_cleanup(
        &mut self,
        commands: &mut Commands,
        reason: &str,
    ) -> TaskHandlerControl {
        self.end_after_custom_cleanup(commands, TaskEndDisposition::AbortedRetryable, reason)
    }

    fn end_after_custom_cleanup(
        &mut self,
        commands: &mut Commands,
        disposition: TaskEndDisposition,
        reason: &str,
    ) -> TaskHandlerControl {
        if let Err(already_ended) = self.try_begin_end(disposition) {
            return self.reject_duplicate_end(reason, already_ended);
        }

        debug!(
            "end_after_custom_cleanup: soul {:?} disposition {:?}",
            self.soul_entity, disposition
        );
        self.release_active_task_reservations();
        crate::soul_ai::execute::task_execution::common::clear_task_and_path(
            &mut self.task,
            &mut self.path,
        );
        self.path_search_progress.clear_entity(self.soul_entity);
        commands
            .entity(self.soul_entity)
            .remove::<(WorkingOn, ActiveTaskIdentity)>();
        TaskHandlerControl::Ended
    }

    /// 専用 cleanup 後、task payload に紐づく予約を context 内で解放して正常終了する。
    fn end_with_assignment_cleanup(
        &mut self,
        commands: &mut Commands,
        disposition: TaskEndDisposition,
        reason: &str,
    ) -> TaskHandlerControl {
        if let Err(already_ended) = self.try_begin_end(disposition) {
            return self.reject_duplicate_end(reason, already_ended);
        }

        use crate::soul_ai::helpers::work::{SoulDropCtx, cleanup_task_assignment};

        let soul_entity = self.soul_entity;
        let drop_pos = self.soul_pos();
        let world_map = self.env.world_map;
        cleanup_task_assignment(
            commands,
            SoulDropCtx {
                soul_entity,
                drop_pos,
                inventory: Some(&mut *self.inventory),
                dropped_item_res: None,
            },
            &mut self.task,
            &mut self.path,
            self.queries,
            world_map,
            false,
        );
        self.path_search_progress.clear_entity(soul_entity);
        commands.entity(soul_entity).remove::<WorkingOn>();
        TaskHandlerControl::Ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_state_keeps_the_first_disposition() {
        let mut state = TaskEndState::default();

        assert_eq!(
            state.try_begin_end(TaskEndDisposition::AbortedRetryable),
            Ok(())
        );
        assert_eq!(
            state.try_begin_end(TaskEndDisposition::Completed),
            Err(AlreadyEnded {
                existing: TaskEndDisposition::AbortedRetryable,
            })
        );
        assert!(!state.is_completed());
    }
}
