use bevy::prelude::*;

use crate::app_contexts::{BuildContext, TaskContext, ZoneContext};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::events::FamiliarOperationMaxSoulChangedEvent;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::EntityListNodeIndex;
use crate::interface::ui::InfoPanelPinState;
use crate::interface::ui::components::{MenuState, OperationDialog};
use crate::relationships::Commanding;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::time::TimeSpeed;
use hw_core::game_state::PlayMode;
use hw_ui::UiIntent;

pub(crate) fn handle_ui_intent(
    mut ui_intents: MessageReader<UiIntent>,
    mut menu_state: ResMut<MenuState>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut build_context: ResMut<BuildContext>,
    mut zone_context: ResMut<ZoneContext>,
    mut task_context: ResMut<TaskContext>,
    mut selected_entity: ResMut<SelectedEntity>,
    mut info_panel_pin: ResMut<InfoPanelPinState>,
    mut q_familiar_ops: Query<&mut FamiliarOperation>,
    q_familiar_meta: Query<(&Familiar, &FamiliarAiState, Option<&Commanding>)>,
    q_familiars_for_area: Query<(Entity, Option<&TaskArea>), With<Familiar>>,
    mut q_dialog: Query<&mut Node, With<OperationDialog>>,
    node_index: Res<EntityListNodeIndex>,
    mut q_text: Query<&mut Text>,
    mut ev_max_soul_changed: MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
    mut time: ResMut<Time<Virtual>>,
) {
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
                super::dialog::open_operation_dialog(&mut q_dialog);
            }
            UiIntent::CloseDialog => {
                super::dialog::close_operation_dialog(&mut q_dialog);
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
    if let Some(selected) = selected {
        if let Ok(mut op) = q_familiar_ops.get_mut(selected) {
            let new_val = (op.fatigue_threshold + delta).clamp(0.0, 1.0);
            op.fatigue_threshold = (new_val * 10.0).round() / 10.0;
        }
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
    if let Some(selected) = selected {
        if let Ok(mut op) = q_familiar_ops.get_mut(selected) {
            let old_val = op.max_controlled_soul;
            let new_val = (old_val as isize + delta).clamp(1, 8) as usize;
            if old_val == new_val {
                return;
            }
            op.max_controlled_soul = new_val;
            update_familiar_max_soul_header(
                selected,
                new_val,
                q_familiar_meta,
                node_index,
                q_text,
            );
            ev_max_soul_changed.write(FamiliarOperationMaxSoulChangedEvent {
                familiar_entity: selected,
                old_value: old_val,
                new_value: new_val,
            });
        }
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
