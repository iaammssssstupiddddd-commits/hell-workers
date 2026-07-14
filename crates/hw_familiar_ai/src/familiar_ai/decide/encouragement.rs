//! 使い魔の激励ロジック（hw_ai）
//!
//! 対象選定ロジック、`EncouragementCooldown` コンポーネント、および
//! `encouragement_decision_system` を提供します。

use bevy::prelude::*;
use hw_core::constants::{ENCOURAGEMENT_INTERVAL_MAX, ENCOURAGEMENT_INTERVAL_MIN};
use hw_core::events::EncouragementRequest;
use hw_core::familiar::{ActiveCommand, Familiar, FamiliarAiState, FamiliarCommand};
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::{FixedAuditSeed, SimulationRandomState, SimulationRng};
use hw_spatial::SpatialGrid;
use hw_world::SpatialGridOps;
use rand::Rng;
use rand::seq::SliceRandom;

use super::FamiliarDecideOutput;
use super::query_types::SoulEncouragementQuery;

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
    /// fixed-step auditではSpatialGridの内部順序に選択結果を委ねない。
    #[cfg(feature = "profiling")]
    pub stable_target_order: bool,
}

#[cfg(feature = "profiling")]
const ENCOURAGEMENT_TARGET_STREAM: u64 = 0x656e_636f_7572_6167;

/// 激励対象を 1 体選ぶ。
///
/// request message の発行は行わず、選ばれた `Entity` を返す。
pub fn decide_encouragement_target<G: SpatialGridOps, R: Rng + ?Sized>(
    ctx: &FamiliarEncouragementContext<'_, '_, '_, G>,
    rng: &mut R,
    scratch: &mut Vec<Entity>,
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

    ctx.soul_grid
        .get_nearby_in_radius_into(ctx.fam_pos, ctx.command_radius, scratch);
    let valid_targets: Vec<Entity> = scratch
        .iter()
        .filter_map(|&soul_entity| {
            let (entity, has_cooldown) = ctx.q_souls.get(soul_entity).ok()?;
            (!has_cooldown).then_some(entity)
        })
        .collect();

    #[cfg(feature = "profiling")]
    let mut valid_targets = valid_targets;
    #[cfg(feature = "profiling")]
    if ctx.stable_target_order {
        valid_targets.sort_unstable_by_key(|entity| entity.to_bits());
    }

    valid_targets.choose(rng).copied()
}

/// 激励要求を生成するシステム（Decide Phase）
pub fn encouragement_decision_system(
    time: Res<Time>,
    #[cfg(feature = "profiling")] audit_seed: Option<Res<FixedAuditSeed>>,
    q_familiars: Query<(
        Entity,
        &GlobalTransform,
        &Familiar,
        &FamiliarAiState,
        &ActiveCommand,
    )>,
    q_souls: SoulEncouragementQuery,
    soul_grid: Res<SpatialGrid>,
    mut nearby_buf: Local<Vec<Entity>>,
    mut decide_output: FamiliarDecideOutput,
    #[cfg(feature = "profiling")] mut random_states: Query<&mut SimulationRandomState>,
) {
    let dt = time.delta_secs();
    #[cfg(not(feature = "profiling"))]
    let mut rng = rand::thread_rng();

    for (fam_entity, fam_transform, familiar, state, active_cmd) in q_familiars.iter() {
        #[cfg(feature = "profiling")]
        let mut random_state = random_states.get_mut(fam_entity).ok();
        #[cfg(feature = "profiling")]
        let mut rng = SimulationRng::for_actor(
            audit_seed.as_deref(),
            random_state.as_deref_mut(),
            ENCOURAGEMENT_TARGET_STREAM,
        );
        let encouragement_ctx = FamiliarEncouragementContext {
            dt,
            ai_state: state,
            active_command: active_cmd,
            fam_pos: fam_transform.translation().truncate(),
            command_radius: familiar.command_radius,
            soul_grid: &*soul_grid,
            q_souls: &q_souls,
            #[cfg(feature = "profiling")]
            stable_target_order: audit_seed.is_some(),
        };

        if let Some(target_soul) =
            decide_encouragement_target(&encouragement_ctx, &mut rng, &mut nearby_buf)
        {
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
