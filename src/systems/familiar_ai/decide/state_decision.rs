use crate::entities::damned_soul::StressBreakdown;
use crate::entities::familiar::FamiliarCommand;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::familiar_ai::decide::FamiliarDecideOutput;
use crate::systems::familiar_ai::helpers::familiar_processor::{
    FamiliarRecruitmentContext, FamiliarSquadContext, finalize_state_transitions,
    process_recruitment, process_squad_management,
};
use crate::systems::familiar_ai::helpers::query_types::{FamiliarSoulQuery, FamiliarStateQuery};
use crate::systems::familiar_ai::perceive::state_detection::determine_transition_reason;
use crate::systems::spatial::SpatialGrid;
use crate::systems::visual::speech::components::{FamiliarBubble, SpeechBubble};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// 使い魔AIの状態決定に必要なSystemParam
#[derive(SystemParam)]
pub struct FamiliarAiStateDecisionParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub time: Res<'w, Time>,
    pub spatial_grid: Res<'w, SpatialGrid>,
    pub q_familiars: FamiliarStateQuery<'w, 's>,
    pub q_souls: FamiliarSoulQuery<'w, 's>,
    pub q_breakdown: Query<'w, 's, &'static StressBreakdown>,
    pub game_assets: Res<'w, crate::assets::GameAssets>,
    pub q_bubbles: Query<'w, 's, (Entity, &'static SpeechBubble), With<FamiliarBubble>>,
    pub decide_output: FamiliarDecideOutput<'w>,
}

/// 使い魔AIの状態更新システム（Decide Phase）
pub fn familiar_ai_state_system(params: FamiliarAiStateDecisionParams) {
    let FamiliarAiStateDecisionParams {
        mut commands,
        time,
        spatial_grid,
        mut q_familiars,
        mut q_souls,
        q_breakdown,
        mut decide_output,
        q_bubbles,
        game_assets,
        ..
    } = params;

    for (
        fam_entity,
        fam_transform,
        familiar,
        familiar_op,
        active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        voice_opt,
        mut history_opt,
    ) in q_familiars.iter_mut()
    {
        debug!(
            "FAM_AI: {:?} Processing. Command: {:?}, State: {:?}, Area: {}",
            fam_entity,
            active_command.command,
            *ai_state,
            task_area_opt.is_some()
        );

        if matches!(active_command.command, FamiliarCommand::Idle) {
            let transition_result =
                crate::systems::familiar_ai::helpers::state_handlers::idle::handle_idle_state(
                    fam_entity,
                    fam_transform,
                    active_command,
                    &mut ai_state,
                    &mut fam_dest,
                    &mut fam_path,
                    &mut commands,
                    &time,
                    &game_assets,
                    &q_bubbles,
                    history_opt.as_deref_mut(),
                    voice_opt,
                );
            if transition_result.apply_to(&mut ai_state) {
                debug!("FAM_AI: {:?} state changed to Idle", fam_entity);
            }
            continue;
        }

        let old_state = ai_state.clone();
        let mut state_changed = false;
        let fam_pos = fam_transform.translation.truncate();
        let fatigue_threshold = familiar_op.fatigue_threshold;
        let max_workers = familiar_op.max_controlled_soul;

        let mut squad_ctx = FamiliarSquadContext {
            fam_entity,
            familiar_op,
            commanding,
            q_souls: &q_souls,
            request_writer: &mut decide_output.squad_requests,
        };
        let mut squad_entities = process_squad_management(&mut squad_ctx);

        match *ai_state {
            FamiliarAiState::Scouting { target_soul } => {
                let mut scouting_ctx =
                    crate::systems::familiar_ai::helpers::scouting::FamiliarScoutingContext {
                        fam_entity,
                        fam_pos,
                        target_soul,
                        fatigue_threshold,
                        max_workers,
                        squad: &mut squad_entities,
                        ai_state: &mut ai_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        q_souls: &mut q_souls,
                        q_breakdown: &q_breakdown,
                        request_writer: &mut decide_output.squad_requests,
                    };
                let transition_result = crate::systems::familiar_ai::helpers::state_handlers::scouting::handle_scouting_state(&mut scouting_ctx);
                state_changed = transition_result.apply_to(&mut ai_state);
            }
            _ => {
                let mut recruitment_ctx = FamiliarRecruitmentContext {
                    fam_entity,
                    fam_transform,
                    familiar,
                    familiar_op,
                    ai_state: &mut ai_state,
                    fam_dest: &mut fam_dest,
                    fam_path: &mut fam_path,
                    squad_entities: &mut squad_entities,
                    max_workers,
                    spatial_grid: &spatial_grid,
                    q_souls: &mut q_souls,
                    q_breakdown: &q_breakdown,
                    request_writer: &mut decide_output.squad_requests,
                };

                if process_recruitment(&mut recruitment_ctx) {
                    state_changed = true;
                }
            }
        }

        if finalize_state_transitions(&mut ai_state, &squad_entities, fam_entity, max_workers) {
            state_changed = true;
        }

        if state_changed {
            decide_output
                .state_requests
                .write(crate::events::FamiliarStateRequest {
                    familiar_entity: fam_entity,
                    new_state: ai_state.clone(),
                });
            decide_output
                .state_changed_events
                .write(crate::events::FamiliarAiStateChangedEvent {
                    familiar_entity: fam_entity,
                    from: old_state.clone(),
                    to: ai_state.clone(),
                    reason: determine_transition_reason(&old_state, &*ai_state),
                });
        }
    }
}
