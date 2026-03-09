use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path};
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarOperation};
use crate::relationships::ParticipatingIn;
use crate::relationships::{CommandedBy, Commanding, ManagedTasks};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;
pub use hw_ai::familiar_ai::decide::query_types::{
    SoulEncouragementQuery, SoulRecruitmentQuery, SoulScoutingQuery, SoulSquadQuery,
    SoulSupervisingQuery,
};

/// 使い魔AIが扱うソウルの標準クエリ型
///
/// タプルの不一致による型エラーを避けるため、単一の型に集約する。
/// task_delegation / task_management / squad_apply など full-fat アクセスが必要な箇所で使用する。
pub type FamiliarSoulQuery<'w, 's> = Query<
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
        Option<&'static mut crate::systems::logistics::Inventory>,
        Option<&'static CommandedBy>,
        Option<&'static ParticipatingIn>,
    ),
    Without<crate::entities::familiar::Familiar>,
>;

/// 使い魔AI状態システム用クエリ型（state_decision.rs 専用）
///
/// speech/UI 依存型（FamiliarVoice / SpeechHistory）を除外した純 AI 版。
pub type FamiliarStateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Familiar,
        &'static FamiliarOperation,
        &'static ActiveCommand,
        &'static mut FamiliarAiState,
        &'static mut Destination,
        &'static mut Path,
        Option<&'static TaskArea>,
        Option<&'static Commanding>,
    ),
>;

/// 使い魔AIタスク委譲システム用クエリ型（FamiliarAiTaskParams用）
pub type FamiliarTaskQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static FamiliarOperation,
        &'static ActiveCommand,
        &'static mut FamiliarAiState,
        &'static mut Destination,
        &'static mut Path,
        Option<&'static TaskArea>,
        Option<&'static Commanding>,
        Option<&'static ManagedTasks>,
    ),
    With<Familiar>,
>;
