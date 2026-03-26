use bevy::prelude::*;
use hw_ui::UiIntent;

use super::super::intent_context::{IntentFamiliarQueries, IntentSelectionCtx, IntentUiQueries};
use crate::FamiliarOperationMaxSoulChangedEvent;
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::ui::EntityListNodeIndex;
use crate::systems::familiar_ai::FamiliarAiState;
use hw_core::relationships::Commanding;

pub(crate) fn handle(
    intent: UiIntent,
    sel_ctx: &mut IntentSelectionCtx<'_>,
    familiar_queries: &mut IntentFamiliarQueries<'_, '_>,
    ui_queries: &mut IntentUiQueries<'_, '_>,
    ev_max_soul_changed: &mut MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
) {
    let selected = sel_ctx.selected_entity.0;
    match intent {
        UiIntent::AdjustFatigueThreshold(delta) => {
            adjust_fatigue_threshold(selected, &mut familiar_queries.q_familiar_ops, delta);
        }
        UiIntent::AdjustMaxControlledSoul(delta) => {
            adjust_max_controlled_soul(
                selected,
                &mut familiar_queries.q_familiar_ops,
                &familiar_queries.q_familiar_meta,
                &sel_ctx.node_index,
                &mut ui_queries.q_text,
                delta,
                ev_max_soul_changed,
            );
        }
        UiIntent::AdjustMaxControlledSoulFor(familiar, delta) => {
            adjust_max_controlled_soul(
                Some(familiar),
                &mut familiar_queries.q_familiar_ops,
                &familiar_queries.q_familiar_meta,
                &sel_ctx.node_index,
                &mut ui_queries.q_text,
                delta,
                ev_max_soul_changed,
            );
        }
        _ => {}
    }
}

fn adjust_fatigue_threshold(
    selected: Option<Entity>,
    q_familiar_ops: &mut Query<&mut FamiliarOperation>,
    delta: f32,
) {
    if let Some(selected) = selected
        && let Ok(mut op) = q_familiar_ops.get_mut(selected)
    {
        let new_val = (op.fatigue_threshold + delta).clamp(0.0, 1.0);
        op.fatigue_threshold = (new_val * 10.0).round() / 10.0;
    }
}

fn adjust_max_controlled_soul(
    selected: Option<Entity>,
    q_familiar_ops: &mut Query<&mut FamiliarOperation>,
    q_familiar_meta: &Query<(&Familiar, &FamiliarAiState, Option<&Commanding>)>,
    node_index: &EntityListNodeIndex,
    q_text: &mut Query<&mut Text>,
    delta: isize,
    ev_max_soul_changed: &mut MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
) {
    if let Some(selected) = selected
        && let Ok(mut op) = q_familiar_ops.get_mut(selected)
    {
        let old_val = op.max_controlled_soul;
        let new_val = (old_val as isize + delta).clamp(1, 8) as usize;
        if old_val == new_val {
            return;
        }
        op.max_controlled_soul = new_val;
        update_familiar_max_soul_header(selected, new_val, q_familiar_meta, node_index, q_text);
        ev_max_soul_changed.write(FamiliarOperationMaxSoulChangedEvent {
            familiar_entity: selected,
            old_value: old_val,
            new_value: new_val,
        });
    }
}

fn update_familiar_max_soul_header(
    familiar_entity: Entity,
    new_val: usize,
    q_familiar_meta: &Query<(&Familiar, &FamiliarAiState, Option<&Commanding>)>,
    node_index: &EntityListNodeIndex,
    q_text: &mut Query<&mut Text>,
) {
    let Some(nodes) = node_index.familiar_sections.get(&familiar_entity) else {
        return;
    };
    let Ok((familiar, ai_state, commanding_opt)) = q_familiar_meta.get(familiar_entity) else {
        return;
    };
    let Ok(mut text) = q_text.get_mut(nodes.header_text) else {
        return;
    };

    let squad_count = commanding_opt.map(|c| c.len()).unwrap_or(0);
    text.0 = format!(
        "{} ({}/{}) [{}]",
        familiar.name,
        squad_count,
        new_val,
        familiar_state_label(ai_state)
    );
}

fn familiar_state_label(ai_state: &FamiliarAiState) -> &'static str {
    match ai_state {
        FamiliarAiState::Idle => "Idle",
        FamiliarAiState::SearchingTask => "Searching",
        FamiliarAiState::Scouting { .. } => "Scouting",
        FamiliarAiState::Supervising { .. } => "Supervising",
    }
}
