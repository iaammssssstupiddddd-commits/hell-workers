use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::FamiliarOperationMaxSoulChangedEvent;
use crate::app_contexts::{BuildContext, TaskContext, ZoneContext};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::EntityListNodeIndex;
use crate::interface::ui::InfoPanelPinState;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::familiar_ai::FamiliarAiState;
use hw_core::game_state::{PlayMode, TimeSpeed};
use hw_core::relationships::Commanding;
use hw_ui::UiIntent;
use hw_ui::components::{MenuState, OperationDialog};

#[derive(SystemParam)]
pub(crate) struct IntentModeCtx<'w> {
    menu_state: ResMut<'w, MenuState>,
    next_play_mode: ResMut<'w, NextState<PlayMode>>,
    build_context: ResMut<'w, BuildContext>,
    zone_context: ResMut<'w, ZoneContext>,
    task_context: ResMut<'w, TaskContext>,
}

#[derive(SystemParam)]
pub(crate) struct IntentSelectionCtx<'w> {
    selected_entity: ResMut<'w, SelectedEntity>,
    info_panel_pin: ResMut<'w, InfoPanelPinState>,
    node_index: Res<'w, EntityListNodeIndex>,
}

#[derive(SystemParam)]
pub(crate) struct IntentFamiliarQueries<'w, 's> {
    q_familiar_ops: Query<'w, 's, &'static mut FamiliarOperation>,
    q_familiar_meta: Query<
        'w,
        's,
        (&'static Familiar, &'static FamiliarAiState, Option<&'static Commanding>),
    >,
    q_familiars_for_area: Query<'w, 's, (Entity, Option<&'static TaskArea>), With<Familiar>>,
}

#[derive(SystemParam)]
pub(crate) struct IntentUiQueries<'w, 's> {
    q_dialog: Query<'w, 's, &'static mut Node, With<OperationDialog>>,
    q_text: Query<'w, 's, &'static mut Text>,
}

pub(crate) fn handle_ui_intent(
    mut ui_intents: MessageReader<UiIntent>,
    mode_ctx: IntentModeCtx,
    selection_ctx: IntentSelectionCtx,
    familiar_queries: IntentFamiliarQueries,
    ui_queries: IntentUiQueries,
    mut ev_max_soul_changed: MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
    mut time: ResMut<Time<Virtual>>,
) {
    let IntentModeCtx {
        mut menu_state,
        mut next_play_mode,
        mut build_context,
        mut zone_context,
        mut task_context,
    } = mode_ctx;
    let IntentSelectionCtx {
        mut selected_entity,
        mut info_panel_pin,
        node_index,
    } = selection_ctx;
    let IntentFamiliarQueries {
        mut q_familiar_ops,
        q_familiar_meta,
        q_familiars_for_area,
    } = familiar_queries;
    let IntentUiQueries {
        mut q_dialog,
        mut q_text,
    } = ui_queries;
    for intent in ui_intents.read().cloned() {
        match intent {
            UiIntent::InspectEntity(entity) => {
                selected_entity.0 = Some(entity);
                info_panel_pin.entity = Some(entity);
            }
            UiIntent::ClearInspectPin => {
                info_panel_pin.entity = None;
            }
            UiIntent::ToggleArchitect => {
                super::mode::toggle_menu_and_reset_mode(
                    &mut menu_state,
                    MenuState::Architect,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                    false,
                );
            }
            UiIntent::ToggleOrders => {
                super::mode::toggle_menu_and_reset_mode(
                    &mut menu_state,
                    MenuState::Orders,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                    true,
                );
            }
            UiIntent::ToggleZones => {
                super::mode::toggle_menu_and_reset_mode(
                    &mut menu_state,
                    MenuState::Zones,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                    true,
                );
            }
            UiIntent::ToggleDream => {
                super::mode::toggle_menu_and_reset_mode(
                    &mut menu_state,
                    MenuState::Dream,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                    false,
                );
            }
            UiIntent::SelectBuild(kind) => {
                super::mode::set_build_mode(
                    kind,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                );
            }
            UiIntent::SelectFloorPlace => {
                super::mode::set_floor_place_mode(
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                );
            }
            UiIntent::SelectZone(kind) => {
                super::mode::set_zone_mode(
                    kind,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                );
            }
            UiIntent::RemoveZone(kind) => {
                super::mode::set_zone_removal_mode(
                    kind,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                );
            }
            UiIntent::SelectTaskMode(mode) => {
                ensure_familiar_selected(
                    &mut selected_entity,
                    &q_familiars_for_area,
                    "Task designation",
                );
                super::mode::set_task_mode(
                    mode,
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                );
            }
            UiIntent::SelectAreaTask => {
                ensure_familiar_selected(&mut selected_entity, &q_familiars_for_area, "Area Edit");
                super::mode::set_area_task_mode(
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                );
            }
            UiIntent::SelectDreamPlanting => {
                super::mode::set_task_mode(
                    TaskMode::DreamPlanting(None),
                    &mut next_play_mode,
                    &mut build_context,
                    &mut zone_context,
                    &mut task_context,
                );
            }
            UiIntent::OpenOperationDialog => {
                hw_ui::interaction::dialog::open_operation_dialog(&mut q_dialog);
            }
            UiIntent::CloseDialog => {
                hw_ui::interaction::dialog::close_operation_dialog(&mut q_dialog);
            }
            UiIntent::AdjustFatigueThreshold(delta) => {
                adjust_fatigue_threshold(selected_entity.0, &mut q_familiar_ops, delta);
            }
            UiIntent::AdjustMaxControlledSoul(delta) => {
                adjust_max_controlled_soul(
                    selected_entity.0,
                    &mut q_familiar_ops,
                    &q_familiar_meta,
                    &node_index,
                    &mut q_text,
                    delta,
                    &mut ev_max_soul_changed,
                );
            }
            UiIntent::AdjustMaxControlledSoulFor(familiar, delta) => {
                adjust_max_controlled_soul(
                    Some(familiar),
                    &mut q_familiar_ops,
                    &q_familiar_meta,
                    &node_index,
                    &mut q_text,
                    delta,
                    &mut ev_max_soul_changed,
                );
            }
            UiIntent::TogglePause => {
                if time.is_paused() {
                    time.unpause();
                } else {
                    time.pause();
                }
            }
            UiIntent::SetTimeSpeed(speed) => match speed {
                TimeSpeed::Paused => time.pause(),
                TimeSpeed::Normal => {
                    time.unpause();
                    time.set_relative_speed(1.0);
                }
                TimeSpeed::Fast => {
                    time.unpause();
                    time.set_relative_speed(2.0);
                }
                TimeSpeed::Super => {
                    time.unpause();
                    time.set_relative_speed(4.0);
                }
            },
            UiIntent::ToggleDoorLock(_)
            | UiIntent::SelectArchitectCategory(_)
            | UiIntent::MovePlantBuilding(_) => {
                // 専用システム側で扱うためここでは無視
            }
        }
    }
}

fn ensure_familiar_selected(
    selected_entity: &mut ResMut<crate::interface::selection::SelectedEntity>,
    q_familiars_for_area: &Query<(Entity, Option<&TaskArea>), With<Familiar>>,
    _mode_label: &str,
) {
    let selected_is_familiar = selected_entity
        .0
        .is_some_and(|entity| q_familiars_for_area.get(entity).is_ok());

    if selected_is_familiar {
        return;
    }

    let mut familiars: Vec<(Entity, bool)> = q_familiars_for_area
        .iter()
        .map(|(entity, area_opt)| (entity, area_opt.is_some()))
        .collect();
    familiars.sort_by_key(|(entity, _)| entity.index());

    let fallback = familiars
        .iter()
        .find(|(_, has_area)| !*has_area)
        .map(|(entity, _)| *entity)
        .or_else(|| familiars.first().map(|(entity, _)| *entity));

    if let Some(familiar_entity) = fallback {
        selected_entity.0 = Some(familiar_entity);
    }
}

fn adjust_fatigue_threshold(
    selected: Option<Entity>,
    q_familiar_ops: &mut Query<&mut FamiliarOperation>,
    delta: f32,
) {
    if let Some(selected) = selected
        && let Ok(mut op) = q_familiar_ops.get_mut(selected) {
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
        && let Ok(mut op) = q_familiar_ops.get_mut(selected) {
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
