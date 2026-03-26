use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::events::OnStressBreakdown;
use hw_core::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use hw_jobs::AssignedTask;
use hw_spatial::FamiliarSpatialGrid;
use hw_world::SpatialGridOps;

type SoulVitalsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut DamnedSoul,
        &'static AssignedTask,
        &'static IdleState,
        Option<&'static CommandedBy>,
        Option<&'static mut StressBreakdown>,
    ),
>;

/// Familiar影響関連の更新を1パスで処理する統合システム
pub fn familiar_influence_unified_system(
    mut commands: Commands,
    time: Res<Time>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    mut nearby_buf: Local<Vec<Entity>>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand)>,
    mut q_souls: SoulVitalsQuery<'_, '_>,
) {
    let dt = time.delta_secs();
    let familiar_search_radius = TILE_SIZE * 15.0;
    let supervision_eval_radius_sq = (TILE_SIZE * 10.0).powi(2);

    for (entity, soul_transform, mut soul, task, idle, under_command, breakdown_opt) in
        q_souls.iter_mut()
    {
        let soul_pos = soul_transform.translation.truncate();
        let has_task = !matches!(*task, AssignedTask::None);
        let is_gathering = matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        );

        familiar_grid.get_nearby_in_radius_into(soul_pos, familiar_search_radius, &mut nearby_buf);
        let mut best_influence = 0.0_f32;
        let mut is_influence_close = false;

        for &fam_entity in nearby_buf.iter() {
            let Ok((fam_transform, familiar, command)) = q_familiars.get(fam_entity) else {
                continue;
            };

            let fam_pos = fam_transform.translation.truncate();
            let distance_sq = soul_pos.distance_squared(fam_pos);
            let command_radius_sq = familiar.command_radius * familiar.command_radius;

            if distance_sq >= command_radius_sq {
                continue;
            }

            is_influence_close = true;

            if distance_sq > supervision_eval_radius_sq {
                continue;
            }

            let distance = distance_sq.sqrt();
            let command_multiplier = if matches!(command.command, FamiliarCommand::Idle) {
                SUPERVISION_IDLE_MULTIPLIER
            } else {
                1.0
            };
            let command_distance_factor = 1.0 - (distance / familiar.command_radius);
            let influence = familiar.efficiency * command_distance_factor * command_multiplier;
            best_influence = best_influence.max(influence);
        }

        let dream_stress_factor = 1.0 + soul.dream * DREAM_STRESS_MULTIPLIER;
        if has_task {
            soul.stress = (soul.stress + dt * STRESS_WORK_RATE * dream_stress_factor).min(1.0);
        } else if under_command.is_some() {
            // 待機中（使役下）ではストレス変化なし
        } else if is_influence_close {
            soul.stress =
                (soul.stress + dt * ESCAPE_PROXIMITY_STRESS_RATE * dream_stress_factor).min(1.0);
        } else if is_gathering {
            soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_GATHERING).max(0.0);
        } else {
            soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_IDLE).max(0.0);
        }

        if has_task && best_influence > 0.0 {
            let supervision_stress =
                best_influence * dt * SUPERVISION_STRESS_SCALE * dream_stress_factor;
            soul.stress = (soul.stress + supervision_stress).min(1.0);
        }

        if best_influence > 0.0 {
            soul.motivation =
                (soul.motivation + best_influence * dt * SUPERVISION_MOTIVATION_SCALE).min(1.0);
            soul.laziness =
                (soul.laziness - best_influence * dt * SUPERVISION_LAZINESS_SCALE).max(0.0);
        } else if has_task || under_command.is_some() {
            soul.motivation = (soul.motivation - dt * MOTIVATION_LOSS_RATE_ACTIVE).max(0.0);
            soul.laziness = (soul.laziness - dt * LAZINESS_LOSS_RATE_ACTIVE).max(0.0);
        } else {
            soul.motivation = (soul.motivation - dt * MOTIVATION_LOSS_RATE_IDLE).max(0.0);
            soul.laziness = (soul.laziness + dt * LAZINESS_GAIN_RATE_IDLE).min(1.0);
        }

        if soul.stress >= 1.0 && breakdown_opt.is_none() {
            commands.trigger(OnStressBreakdown { entity });
        }

        if let Some(mut breakdown) = breakdown_opt {
            if soul.stress <= STRESS_RECOVERY_THRESHOLD {
                commands.entity(entity).remove::<StressBreakdown>();
            } else if breakdown.is_frozen {
                breakdown.remaining_freeze_secs = (breakdown.remaining_freeze_secs - dt).max(0.0);
                if breakdown.remaining_freeze_secs <= 0.0 {
                    breakdown.is_frozen = false;
                }
            } else if breakdown.remaining_freeze_secs > 0.0 {
                breakdown.remaining_freeze_secs = (breakdown.remaining_freeze_secs - dt).max(0.0);
            }
        }
    }
}
