use bevy::prelude::*;

use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::events::FamiliarOperationMaxSoulChangedEvent;
use crate::game_state::{BuildContext, PlayMode, TaskContext, ZoneContext};
use crate::interface::ui::InfoPanelPinState;
use crate::interface::ui::components::{MenuAction, MenuState, OperationDialog};
use crate::systems::command::TaskArea;
use crate::systems::time::TimeSpeed;

pub(super) fn handle_pressed_action(
    action: MenuAction,
    menu_state: &mut ResMut<MenuState>,
    next_play_mode: &mut ResMut<NextState<PlayMode>>,
    build_context: &mut ResMut<BuildContext>,
    zone_context: &mut ResMut<ZoneContext>,
    task_context: &mut ResMut<TaskContext>,
    selected_entity: &mut ResMut<crate::interface::selection::SelectedEntity>,
    info_panel_pin: &mut ResMut<InfoPanelPinState>,
    q_familiar_ops: &mut Query<&mut FamiliarOperation>,
    q_familiars_for_area: &Query<(Entity, Option<&TaskArea>), With<Familiar>>,
    q_dialog: &mut Query<&mut Node, With<OperationDialog>>,
    ev_max_soul_changed: &mut MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
    time: &mut ResMut<Time<Virtual>>,
) {
    // MenuAction は責務で分類して扱う。
    // 汎用処理: `ui_interaction_system` の入口で一括処理する。
    // 専用処理: 各専用システム (`arch_category_action_system` / `door_lock_action_system`) が担う。
    if action.is_specialized() {
        return;
    }

    match action {
        MenuAction::InspectEntity(entity) => {
            selected_entity.0 = Some(entity);
            info_panel_pin.entity = Some(entity);
        }
        MenuAction::ClearInspectPin => {
            info_panel_pin.entity = None;
        }
        MenuAction::ToggleArchitect => super::mode::toggle_menu_and_reset_mode(
            menu_state,
            MenuState::Architect,
            next_play_mode,
            build_context,
            zone_context,
            task_context,
            false,
        ),
        MenuAction::ToggleOrders => super::mode::toggle_menu_and_reset_mode(
            menu_state,
            MenuState::Orders,
            next_play_mode,
            build_context,
            zone_context,
            task_context,
            true,
        ),
        MenuAction::ToggleZones => super::mode::toggle_menu_and_reset_mode(
            menu_state,
            MenuState::Zones,
            next_play_mode,
            build_context,
            zone_context,
            task_context,
            true,
        ),
        MenuAction::ToggleDream => super::mode::toggle_menu_and_reset_mode(
            menu_state,
            MenuState::Dream,
            next_play_mode,
            build_context,
            zone_context,
            task_context,
            false,
        ),
        MenuAction::SelectBuild(kind) => super::mode::set_build_mode(
            kind,
            next_play_mode,
            build_context,
            zone_context,
            task_context,
        ),
        MenuAction::SelectFloorPlace => super::mode::set_floor_place_mode(
            next_play_mode,
            build_context,
            zone_context,
            task_context,
        ),
        MenuAction::SelectZone(kind) => super::mode::set_zone_mode(
            kind,
            next_play_mode,
            build_context,
            zone_context,
            task_context,
        ),
        MenuAction::RemoveZone(kind) => super::mode::set_zone_removal_mode(
            kind,
            next_play_mode,
            build_context,
            zone_context,
            task_context,
        ),
        MenuAction::SelectTaskMode(mode) => {
            ensure_familiar_selected(selected_entity, q_familiars_for_area, "Task designation");
            super::mode::set_task_mode(
                mode,
                next_play_mode,
                build_context,
                zone_context,
                task_context,
            );
        }
        MenuAction::SelectAreaTask => {
            ensure_familiar_selected(selected_entity, q_familiars_for_area, "Area Edit");

            super::mode::set_area_task_mode(
                next_play_mode,
                build_context,
                zone_context,
                task_context,
            );
        }
        MenuAction::SelectDreamPlanting => {
            super::mode::set_task_mode(
                crate::systems::command::TaskMode::DreamPlanting(None),
                next_play_mode,
                build_context,
                zone_context,
                task_context,
            );
        }
        MenuAction::OpenOperationDialog => super::dialog::open_operation_dialog(q_dialog),
        MenuAction::CloseDialog => super::dialog::close_operation_dialog(q_dialog),
        MenuAction::AdjustFatigueThreshold(delta) => {
            adjust_fatigue_threshold(selected_entity.0, q_familiar_ops, delta);
        }
        MenuAction::AdjustMaxControlledSoul(delta) => {
            adjust_max_controlled_soul(
                selected_entity.0,
                q_familiar_ops,
                delta,
                ev_max_soul_changed,
            );
        }
        MenuAction::TogglePause => {
            if time.is_paused() {
                time.unpause();
            } else {
                time.pause();
            }
        }
        MenuAction::SetTimeSpeed(speed) => match speed {
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
        _ => unreachable!("Unhandled MenuAction in generic action router; specialized actions must be delegated."),
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

    // 1) TaskAreaを持っていないFamiliarを優先
    // 2) 全員持っている場合は任意（Entity index最小）を選択
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
    delta: isize,
    ev_max_soul_changed: &mut MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
) {
    if let Some(selected) = selected {
        if let Ok(mut op) = q_familiar_ops.get_mut(selected) {
            let old_val = op.max_controlled_soul;
            let new_val = (old_val as isize + delta).clamp(1, 8) as usize;
            op.max_controlled_soul = new_val;
            if old_val != new_val {
                ev_max_soul_changed.write(FamiliarOperationMaxSoulChangedEvent {
                    familiar_entity: selected,
                    old_value: old_val,
                    new_value: new_val,
                });
            }
        }
    }
}
