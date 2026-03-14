//! 使い魔の激励ロジック（hw_ai）
//!
//! 対象選定ロジック、`EncouragementCooldown` コンポーネント、および
//! `encouragement_decision_system` を提供します。

use bevy::prelude::*;
use hw_core::constants::{ENCOURAGEMENT_INTERVAL_MAX, ENCOURAGEMENT_INTERVAL_MIN};
use hw_core::events::EncouragementRequest;
use hw_core::familiar::{ActiveCommand, Familiar, FamiliarAiState, FamiliarCommand};
use hw_spatial::SpatialGrid;
use hw_world::SpatialGridOps;
use rand::Rng;
use rand::seq::SliceRandom;

use super::query_types::SoulEncouragementQuery;
use super::FamiliarDecideOutput;

/// 激励のクールダウン管理コンポーネント
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct EncouragementCooldown {
    /// 次回激励可能になる時間（elapsed_secs）
    pub expiry: f32,
}

/// 激励対象選定に必要なコンテキスト
pub struct FamiliarEncouragementContext<'a, 'w, 's, G: SpatialGridOps> {
    pub dt: f32,
    pub ai_state: &'a FamiliarAiState,
    pub active_command: &'a ActiveCommand,
    pub fam_pos: Vec2,
    pub command_radius: f32,
    pub soul_grid: &'a G,
    pub q_souls: &'a SoulEncouragementQuery<'w, 's>,
}

/// 激励対象を 1 体選ぶ。
///
/// request message の発行は行わず、選ばれた `Entity` を返す。
pub fn decide_encouragement_target<G: SpatialGridOps, R: Rng + ?Sized>(
    ctx: &FamiliarEncouragementContext<'_, '_, '_, G>,
    rng: &mut R,
) -> Option<Entity> {
    if !matches!(ctx.ai_state, FamiliarAiState::Supervising { .. }) {
        return None;
    }
    if matches!(ctx.active_command.command, FamiliarCommand::Idle) {
        return None;
    }

    let avg_interval = (ENCOURAGEMENT_INTERVAL_MIN + ENCOURAGEMENT_INTERVAL_MAX) * 0.5;
    let check_chance = (ctx.dt / avg_interval).clamp(0.0, 1.0) as f64;
    if !rng.gen_bool(check_chance) {
        return None;
    }

    let nearby = ctx
        .soul_grid
        .get_nearby_in_radius(ctx.fam_pos, ctx.command_radius);
    let valid_targets: Vec<Entity> = nearby
        .iter()
        .filter_map(|&soul_entity| {
            let (entity, has_cooldown) = ctx.q_souls.get(soul_entity).ok()?;
            (!has_cooldown).then_some(entity)
        })
        .collect();

    valid_targets.choose(rng).copied()
}

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
                .write(EncouragementRequest {
                    familiar_entity: fam_entity,
                    soul_entity: target_soul,
                });
            break;
        }
    }
}
