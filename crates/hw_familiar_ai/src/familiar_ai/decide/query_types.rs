//! hw_ai familiar_ai decide 用クエリ型定義

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, Familiar, FamiliarAiState, FamiliarOperation};
use hw_core::relationships::{CommandedBy, Commanding, ManagedTasks, ParticipatingIn};
use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
use hw_jobs::AssignedTask;
use hw_logistics::Inventory;

use super::encouragement::EncouragementCooldown;

/// 分隊検証・疲労解放に使用する最小クエリ
///
/// `SquadManager` と `process_squad_management` が必要とする
/// fatigue / idle behavior / 帰属確認だけを持つ。
pub type SoulSquadQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static DamnedSoul,
        &'static IdleState,
        Option<&'static CommandedBy>,
    ),
    Without<Familiar>,
>;

/// 監視状態のターゲット選定・追従に使用する最小クエリ
pub type SoulSupervisingQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static AssignedTask), Without<Familiar>>;

/// スカウト状態のターゲット確認に使用する最小クエリ
pub type SoulScoutingQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static DamnedSoul,
        &'static AssignedTask,
        Option<&'static CommandedBy>,
    ),
    Without<Familiar>,
>;

/// リクルート候補フィルタリングに使用する最小クエリ
///
/// `RecruitmentManager::find_best_recruit` / `try_immediate_recruit` /
/// `start_scouting` および `process_recruitment` が必要とするフィールド。
/// `FamiliarSoulQuery`（root、10フィールド）から transmute_lens で派生させて渡す。
pub type SoulRecruitmentQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static DamnedSoul,
        &'static AssignedTask,
        &'static IdleState,
        Option<&'static CommandedBy>,
    ),
    Without<Familiar>,
>;

/// 激励対象フィルタリングに使用する最小クエリ
pub type SoulEncouragementQuery<'w, 's> =
    Query<'w, 's, (Entity, Has<EncouragementCooldown>), With<DamnedSoul>>;

// ──────────────────────────────────────────────────────────────────────────────
// Full-fat クエリ（state_decision.rs / task_delegation.rs で使用）
// ──────────────────────────────────────────────────────────────────────────────

/// 使い魔AI状態システム用クエリ型（state_decision.rs 専用）
///
/// Familiar エンティティ 1 件の全 AI 関連コンポーネントを束ねる full-fat クエリ。
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

/// 使い魔AIが扱うソウルの標準クエリ型
///
/// タスク委譲・分隊管理・task_management など full-fat アクセスが必要な箇所で使用する。
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
        Option<&'static mut Inventory>,
        Option<&'static CommandedBy>,
        Option<&'static ParticipatingIn>,
    ),
    Without<Familiar>,
>;

/// 使い魔AIタスク委譲システム用クエリ型（task_delegation.rs 専用）
///
/// Familiar エンティティの AI 状態・移動・タスク管理コンポーネントを束ねる。
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
