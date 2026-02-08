use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path, StressBreakdown};
use crate::entities::familiar::Familiar;
use crate::relationships::{CommandedBy, WorkingOn};
use crate::systems::logistics::Inventory;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

/// タスク割り当て要求の適用に使うソウルの標準クエリ型
pub type TaskAssignmentSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut AssignedTask,
        &'static mut Destination,
        &'static mut Path,
        &'static IdleState,
        Option<&'static mut Inventory>,
        Option<&'static CommandedBy>,
        Option<&'static crate::systems::soul_ai::gathering::ParticipatingIn>,
    ),
    (With<DamnedSoul>, Without<Familiar>),
>;

/// タスク実行に使うソウルの標準クエリ型
pub type TaskExecutionSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut DamnedSoul,
        &'static mut AssignedTask,
        &'static mut Destination,
        &'static mut Path,
        &'static mut Inventory,
        Option<&'static StressBreakdown>,
    ),
>;

/// Idle 行動の決定に使うソウルの標準クエリ型
pub type IdleDecisionSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut IdleState,
        &'static mut Destination,
        &'static DamnedSoul,
        &'static mut Path,
        &'static AssignedTask,
        Option<&'static ParticipatingIn>,
    ),
    (Without<WorkingOn>, Without<CommandedBy>),
>;

/// Idle の集会分離に使うソウルの標準クエリ型
pub type IdleSeparationSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut Destination,
        &'static mut IdleState,
        &'static Path,
        &'static AssignedTask,
        &'static ParticipatingIn,
    ),
>;

/// Idle 行動のビジュアル更新に使うソウルの標準クエリ型
pub type IdleVisualSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Sprite,
        &'static IdleState,
        &'static DamnedSoul,
        &'static AssignedTask,
        Option<&'static ParticipatingIn>,
    ),
>;

/// 逃走行動に使うソウルの標準クエリ型
pub type EscapingBehaviorSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut IdleState,
        &'static mut Destination,
        &'static mut Path,
        Option<&'static CommandedBy>,
    ),
>;

/// 使役解除のクリーンアップに使うソウルの標準クエリ型
pub type CleanupSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static CommandedBy,
        &'static mut AssignedTask,
        &'static mut Path,
        Option<&'static mut Inventory>,
    ),
>;

/// 建築タスクの自動割り当てに使うソウルの標準クエリ型
pub type AutoBuildSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static DamnedSoul,
        &'static mut AssignedTask,
        &'static mut Destination,
        &'static mut Path,
        &'static IdleState,
        Option<&'static CommandedBy>,
    ),
    Without<Familiar>,
>;

/// オートホールの集計に使うソウルの標準クエリ型
pub type AutoHaulAssignedTaskQuery<'w, 's> = Query<'w, 's, &'static AssignedTask>;
