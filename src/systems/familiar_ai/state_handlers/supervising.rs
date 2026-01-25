//! Supervising 状態のハンドラー
//!
//! 配下の魂を監視し、仕事の進捗を管理している状態の処理を行います。

use super::StateTransitionResult;
use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path};
use crate::entities::familiar::UnderCommand;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

/// Supervising 状態のハンドラー
pub fn handle_supervising_state(
    fam_entity: Entity,
    fam_pos: Vec2,
    active_members: &[Entity],
    task_area_opt: Option<&TaskArea>,
    time: &Res<Time>,
    ai_state: &mut FamiliarAiState,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
    q_souls: &mut Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,

            Option<&mut crate::systems::logistics::Inventory>,
            Option<&UnderCommand>,
            Option<&ParticipatingIn>,
        ),
        Without<crate::entities::familiar::Familiar>,
    >,
    has_available_task: bool,
) -> StateTransitionResult {
    // 既存の supervising_logic を呼び出し
    crate::systems::familiar_ai::supervising::supervising_logic(
        fam_entity,
        fam_pos,
        active_members,
        task_area_opt,
        time,
        ai_state,
        fam_dest,
        fam_path,
        q_souls,
        has_available_task,
    );

    StateTransitionResult::Stay
}
