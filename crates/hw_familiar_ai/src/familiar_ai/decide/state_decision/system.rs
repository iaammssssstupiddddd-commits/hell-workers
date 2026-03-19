use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{RestAreaCooldown, StressBreakdown};
use hw_spatial::SpatialGrid;

use super::super::{
    FamiliarDecideOutput,
    query_types::{FamiliarSoulQuery, FamiliarStateQuery},
};
use super::path::{FamiliarDecisionPath, determine_decision_path};
use super::result::{FamiliarStateDecisionResult, emit_state_decision_messages};
use crate::familiar_ai::decide::{
    helpers::{
        FamiliarSquadContext, SquadManagementOutcome, finalize_state_transitions,
        process_squad_management,
    },
    recruitment::{FamiliarRecruitmentContext, RecruitmentOutcome, process_recruitment},
    scouting::FamiliarScoutingContext,
    state_handlers,
};
use hw_jobs::AssignedTask;

/// 使い魔AIの状態決定に必要なSystemParam
#[derive(SystemParam)]
pub struct FamiliarAiStateDecisionParams<'w, 's> {
    pub spatial_grid: Res<'w, SpatialGrid>,
    pub q_familiars: FamiliarStateQuery<'w, 's>,
    pub q_souls: FamiliarSoulQuery<'w, 's>,
    pub q_breakdown: Query<'w, 's, &'static StressBreakdown>,
    pub q_resting: Query<'w, 's, (), With<hw_core::relationships::RestingIn>>,
    pub q_rest_cooldown: Query<'w, 's, &'static RestAreaCooldown>,
    pub decide_output: FamiliarDecideOutput<'w>,
}

/// 使い魔AIの状態更新システム（Decide Phase）
pub fn familiar_ai_state_system(params: FamiliarAiStateDecisionParams) {
    let FamiliarAiStateDecisionParams {
        spatial_grid,
        mut q_familiars,
        mut q_souls,
        q_breakdown,
        q_resting,
        q_rest_cooldown,
        mut decide_output,
        ..
    } = params;

    // 同フレーム内での重複リクルートを防ぐ予約セット
    let mut recruitment_reservations: HashSet<Entity> = HashSet::new();

    for (
        fam_entity,
        fam_transform,
        familiar,
        familiar_op,
        active_command,
        ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
    ) in q_familiars.iter_mut()
    {
        debug!(
            "FAM_AI: {:?} Processing. Command: {:?}, State: {:?}, Area: {}",
            fam_entity,
            active_command.command,
            *ai_state,
            task_area_opt.is_some()
        );

        let old_state = ai_state.clone();
        let mut next_state = old_state.clone();
        let max_workers = familiar_op.max_controlled_soul;
        let current_count = commanding.map(|c| c.len()).unwrap_or(0);

        let path = determine_decision_path(
            &active_command.command,
            &old_state,
            max_workers,
            current_count,
        );

        let result = match path {
            FamiliarDecisionPath::IdleScoutingContinue { target_soul } => {
                // スカウト継続: ターゲットを予約登録して他 familiar が横取りしないよう保護
                recruitment_reservations.insert(target_soul);
                let fam_pos = fam_transform.translation.truncate();
                let mut squad_entities: Vec<Entity> = commanding
                    .map(|c| c.iter().copied().collect())
                    .unwrap_or_default();
                let transition_result = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarScoutingContext {
                        fam_entity,
                        fam_pos,
                        target_soul,
                        fatigue_threshold: familiar_op.fatigue_threshold,
                        max_workers,
                        squad: &mut squad_entities,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                    };
                    state_handlers::scouting::handle_scouting_state(&mut ctx)
                };
                let recruited = transition_result.recruited_entity;
                let transition_applied = transition_result.transition.apply_to(&mut next_state);
                FamiliarStateDecisionResult::from_idle_scouting(recruited, transition_applied)
            }

            FamiliarDecisionPath::IdleRecruitSearch => {
                // 未スカウト: 即時リクルートまたはスカウト開始
                let mut squad_entities: Vec<Entity> = commanding
                    .map(|c| c.iter().copied().collect())
                    .unwrap_or_default();
                let outcome = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarRecruitmentContext {
                        fam_entity,
                        fam_transform,
                        familiar,
                        familiar_op,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        squad_entities: &mut squad_entities,
                        max_workers,
                        task_area_opt,
                        spatial_grid: &*spatial_grid,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                        q_resting: &q_resting,
                        q_cooldown: &q_rest_cooldown,
                        recruitment_reservations: &mut recruitment_reservations,
                    };
                    process_recruitment(&mut ctx)
                };
                match outcome {
                    RecruitmentOutcome::ImmediateRecruit(soul) => {
                        FamiliarStateDecisionResult::from_idle_recruitment(Some(soul), true)
                    }
                    RecruitmentOutcome::ScoutingStarted => {
                        FamiliarStateDecisionResult::from_idle_recruitment(None, true)
                    }
                    RecruitmentOutcome::NoRecruit => FamiliarStateDecisionResult::no_change(),
                }
            }

            FamiliarDecisionPath::IdleSquadFull => {
                // 分隊十分: Idle 停止ロジック
                let transition = state_handlers::idle::handle_idle_state(
                    active_command,
                    &next_state,
                    fam_transform.translation.truncate(),
                    &mut fam_dest,
                    &mut fam_path,
                );
                let applied = transition.apply_to(&mut next_state);
                FamiliarStateDecisionResult::from_idle_squad_full(applied)
            }

            FamiliarDecisionPath::NonIdleScoutingContinue { target_soul } => {
                let fam_pos = fam_transform.translation.truncate();
                let fatigue_threshold = familiar_op.fatigue_threshold;

                let SquadManagementOutcome {
                    mut squad_entities,
                    released_entities,
                } = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &hw_core::soul::DamnedSoul,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarSquadContext {
                        fam_entity,
                        familiar_op,
                        commanding,
                        q_souls: &q,
                    };
                    process_squad_management(&mut ctx)
                };

                // スカウト中のターゲットを予約登録
                recruitment_reservations.insert(target_soul);
                let transition_result = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarScoutingContext {
                        fam_entity,
                        fam_pos,
                        target_soul,
                        fatigue_threshold,
                        max_workers,
                        squad: &mut squad_entities,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                    };
                    state_handlers::scouting::handle_scouting_state(&mut ctx)
                };
                let recruited = transition_result.recruited_entity;
                let scout_changed = transition_result.transition.apply_to(&mut next_state);
                let finalized = finalize_state_transitions(
                    &mut next_state,
                    &squad_entities,
                    fam_entity,
                    max_workers,
                );
                FamiliarStateDecisionResult::from_non_idle(
                    released_entities,
                    recruited,
                    scout_changed || finalized,
                )
            }

            FamiliarDecisionPath::NonIdleRecruitOrTransition => {
                let fatigue_threshold = familiar_op.fatigue_threshold;
                let _ = fatigue_threshold;

                let SquadManagementOutcome {
                    mut squad_entities,
                    released_entities,
                } = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &hw_core::soul::DamnedSoul,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarSquadContext {
                        fam_entity,
                        familiar_op,
                        commanding,
                        q_souls: &q,
                    };
                    process_squad_management(&mut ctx)
                };

                let outcome = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarRecruitmentContext {
                        fam_entity,
                        fam_transform,
                        familiar,
                        familiar_op,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        squad_entities: &mut squad_entities,
                        max_workers,
                        task_area_opt,
                        spatial_grid: &*spatial_grid,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                        q_resting: &q_resting,
                        q_cooldown: &q_rest_cooldown,
                        recruitment_reservations: &mut recruitment_reservations,
                    };
                    process_recruitment(&mut ctx)
                };
                let (recruited, recruit_changed) = match outcome {
                    RecruitmentOutcome::ImmediateRecruit(e) => (Some(e), true),
                    RecruitmentOutcome::ScoutingStarted => (None, true),
                    RecruitmentOutcome::NoRecruit => (None, false),
                };
                let finalized = finalize_state_transitions(
                    &mut next_state,
                    &squad_entities,
                    fam_entity,
                    max_workers,
                );
                FamiliarStateDecisionResult::from_non_idle(
                    released_entities,
                    recruited,
                    recruit_changed || finalized,
                )
            }
        };

        emit_state_decision_messages(
            fam_entity,
            &old_state,
            &next_state,
            &result,
            &mut decide_output,
        );
    }
}
