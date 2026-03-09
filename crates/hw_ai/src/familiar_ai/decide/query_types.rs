//! hw_ai familiar_ai decide 用クエリ型定義

use bevy::prelude::*;
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleState};
use hw_jobs::AssignedTask;

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
pub type SoulSupervisingQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static AssignedTask),
    Without<Familiar>,
>;

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
