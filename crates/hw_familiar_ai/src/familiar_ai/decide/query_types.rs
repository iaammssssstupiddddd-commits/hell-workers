//! hw_ai familiar_ai decide 用クエリ型定義

use bevy::prelude::*;
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleState};
use hw_jobs::AssignedTask;

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
