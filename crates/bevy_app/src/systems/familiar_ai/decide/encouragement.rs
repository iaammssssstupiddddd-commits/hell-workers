//! 使い魔による激励システム（Decide）
//!
//! 対象選定ロジックは `hw_ai` にあり、本ファイルは root adapter を提供します。

use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::familiar_ai::decide::FamiliarDecideOutput;
use crate::systems::familiar_ai::helpers::query_types::SoulEncouragementQuery;
use crate::systems::spatial::SpatialGrid;
use bevy::prelude::*;

pub use hw_ai::familiar_ai::decide::encouragement::{
    EncouragementCooldown, FamiliarEncouragementContext, decide_encouragement_target,
};

/// 激励要求を生成するシステム（Decide Phase）
pub fn encouragement_decision_system(
    time: Res<Time>,
    q_familiars: Query<(
        Entity,
        &GlobalTransform,
        &Familiar,
        &FamiliarAiState,
        &ActiveCommand,
    )>,
    q_souls: SoulEncouragementQuery,
    soul_grid: Res<SpatialGrid>,
    mut decide_output: FamiliarDecideOutput,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (fam_entity, fam_transform, familiar, state, active_cmd) in q_familiars.iter() {
        let encouragement_ctx = FamiliarEncouragementContext {
            dt,
            ai_state: state,
            active_command: active_cmd,
            fam_pos: fam_transform.translation().truncate(),
            command_radius: familiar.command_radius,
            soul_grid: &*soul_grid,
            q_souls: &q_souls,
        };

        if let Some(target_soul) = decide_encouragement_target(&encouragement_ctx, &mut rng) {
            decide_output
                .encouragement_requests
                .write(crate::events::EncouragementRequest {
                    familiar_entity: fam_entity,
                    soul_entity: target_soul,
                });
            break;
        }
    }
}
