use crate::systems::familiar_ai::FamiliarAiState;
use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path};
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarOperation, FamiliarVoice};
use crate::relationships::{CommandedBy, Commanding, ManagedTasks};
use crate::systems::command::TaskArea;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::visual::speech::cooldown::SpeechHistory;
use bevy::prelude::*;

/// 使い魔AIが扱うソウルの標準クエリ型
///
/// タプルの不一致による型エラーを避けるため、単一の型に集約する。
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

/// 使い魔AI状態システム用クエリ型（FamiliarAiParams用）
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
        Option<&'static FamiliarVoice>,
        Option<&'static mut SpeechHistory>,
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
