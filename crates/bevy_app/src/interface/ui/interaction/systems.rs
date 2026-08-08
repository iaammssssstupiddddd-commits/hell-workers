use crate::entities::familiar::{Familiar, FamiliarOperation, FamiliarPolicy};
use crate::input_actions::ForegroundUiGate;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use hw_jobs::{Building, BuildingCategory};
use hw_ui::UiIntent;
use hw_ui::components::*;
use hw_ui::interaction::HoverActionTarget;
use hw_ui::interaction::common::update_interaction_color;
use hw_ui::interaction::dialog::close_operation_dialog;
use hw_ui::selection::HoveredEntity;
use hw_ui::theme::UiTheme;

use super::menu_actions;

type MenuButtonWithColorQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Interaction,
        &'static MenuButton,
        &'static mut BackgroundColor,
    ),
    (Changed<Interaction>, With<Button>),
>;

type OperationDialogTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        Without<OperationPolicyAllowedText>,
        Without<OperationPolicyPriorityText>,
    ),
>;

type OperationWarningQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<OperationPolicyAllDisabledWarning>,
        Without<OperationDialog>,
    ),
>;

type OperationAllowedButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static OperationPolicyAllowedButton,
        &'static mut MenuButton,
        &'static mut BackgroundColor,
    ),
    Without<OperationPolicyPriorityButton>,
>;

type OperationAllowedTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static OperationPolicyAllowedText, &'static mut Text),
    (
        Without<OperationPolicyPriorityText>,
        Without<OperationPolicyAllowedButton>,
    ),
>;

type OperationPriorityButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static OperationPolicyPriorityButton,
        &'static mut MenuButton,
    ),
    Without<OperationPolicyAllowedButton>,
>;

type OperationPriorityTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static OperationPolicyPriorityText, &'static mut Text),
    (
        Without<OperationPolicyAllowedText>,
        Without<OperationPolicyPriorityButton>,
    ),
>;

#[derive(SystemParam)]
pub struct OperationDialogParams<'w, 's> {
    dialog_state: ResMut<'w, OperationDialogState>,
    ui_nodes: Res<'w, UiNodeRegistry>,
    theme: Res<'w, UiTheme>,
    q_familiars: Query<
        'w,
        's,
        (
            &'static Familiar,
            &'static FamiliarOperation,
            &'static FamiliarPolicy,
        ),
    >,
    q_dialog: Query<'w, 's, &'static mut Node, With<OperationDialog>>,
    q_scroll: Query<'w, 's, &'static mut ScrollPosition, With<OperationDialogScroll>>,
    q_text: OperationDialogTextQuery<'w, 's>,
    q_warning: OperationWarningQuery<'w, 's>,
    q_allowed_buttons: OperationAllowedButtonQuery<'w, 's>,
    q_allowed_text: OperationAllowedTextQuery<'w, 's>,
    q_priority_buttons: OperationPriorityButtonQuery<'w, 's>,
    q_priority_text: OperationPriorityTextQuery<'w, 's>,
}

pub fn update_ui_input_state_system(
    mut ui_input_state: ResMut<UiInputState>,
    q_blockers: Query<&RelativeCursorPosition, With<UiInputBlocker>>,
    q_buttons: Query<&Interaction, With<Button>>,
) {
    let pointer_over_blocker = q_blockers.iter().any(RelativeCursorPosition::cursor_over);
    let pointer_over_button = q_buttons
        .iter()
        .any(|interaction| matches!(*interaction, Interaction::Hovered | Interaction::Pressed));
    ui_input_state.pointer_over_ui = pointer_over_blocker || pointer_over_button;
}

/// UI ボタンの操作を受け取り、`UiIntent` を発行する統合システム
pub fn ui_interaction_system(
    mut interaction_query: MenuButtonWithColorQuery,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    mut commands: Commands,
    mut ui_intent_writer: MessageWriter<UiIntent>,
    theme: Res<UiTheme>,
    foreground_gate: ForegroundUiGate,
) {
    for (entity, interaction, menu_button, mut color) in interaction_query.iter_mut() {
        update_interaction_color(*interaction, &mut color, &theme);
        if *interaction != Interaction::Pressed {
            continue;
        }
        if !foreground_gate.allows(entity) {
            continue;
        }

        super::despawn_context_menus(&mut commands, &q_context_menu);
        menu_actions::handle_pressed_action(entity, menu_button.0, &mut ui_intent_writer);
    }
}

/// Root adapter that publishes only movable Plant buildings to the UI widget.
pub fn update_move_plant_hover_target_system(
    hovered: Res<HoveredEntity>,
    q_buildings: Query<&Building, Without<hw_jobs::DeconstructionPending>>,
    mut target: ResMut<HoverActionTarget>,
) {
    target.0 = hovered.0.filter(|entity| {
        q_buildings
            .get(*entity)
            .is_ok_and(|building| building.kind.category() == BuildingCategory::Plant)
    });
}

/// Operation Dialog のテキスト表示を更新するシステム
pub fn update_operation_dialog_system(params: OperationDialogParams) {
    let OperationDialogParams {
        mut dialog_state,
        ui_nodes,
        theme,
        q_familiars,
        mut q_dialog,
        mut q_scroll,
        mut q_text,
        mut q_warning,
        mut q_allowed_buttons,
        mut q_allowed_text,
        mut q_priority_buttons,
        mut q_priority_text,
    } = params;

    if let Some(target) = dialog_state.target {
        if let Ok((familiar, op, policy)) = q_familiars.get(target) {
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogFamiliarName)
                && let Ok(mut text) = q_text.get_mut(entity)
            {
                text.0 = format!("Editing: {}", familiar.name);
            }
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogThresholdText)
                && let Ok(mut text) = q_text.get_mut(entity)
            {
                let val_str = if op.recruit_fatigue_threshold().is_some() {
                    format!("{:.0}%", op.fatigue_threshold * 100.0)
                } else {
                    "0% (Recruit Off)".to_string()
                };
                if text.0 != val_str {
                    text.0 = val_str;
                }
            }
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogMaxSoulText)
                && let Ok(mut text) = q_text.get_mut(entity)
            {
                let val_str = format!("{}", op.max_controlled_soul);
                if text.0 != val_str {
                    text.0 = val_str;
                }
            }
            for mut node in &mut q_warning {
                node.display = if policy.all_work_disabled() {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            for (marker, mut action, mut color) in &mut q_allowed_buttons {
                let rule = policy.rule_for(marker.0);
                action.0 = UiIntent::ApplyFamiliarSettings {
                    patch: hw_core::familiar::FamiliarSettingsPatch::SetWorkAllowed {
                        work_type: marker.0,
                        allowed: !rule.allowed,
                    },
                };
                color.0 = if rule.allowed {
                    theme.colors.status_healthy
                } else {
                    theme.colors.status_danger
                };
            }
            for (marker, mut text) in &mut q_allowed_text {
                text.0 = if policy.rule_for(marker.0).allowed {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                };
            }
            for (marker, mut action) in &mut q_priority_buttons {
                let next = match policy.rule_for(marker.0).priority {
                    hw_core::familiar::FamiliarWorkPriority::Low => {
                        hw_core::familiar::FamiliarWorkPriority::Normal
                    }
                    hw_core::familiar::FamiliarWorkPriority::Normal => {
                        hw_core::familiar::FamiliarWorkPriority::High
                    }
                    hw_core::familiar::FamiliarWorkPriority::High => {
                        hw_core::familiar::FamiliarWorkPriority::Low
                    }
                };
                action.0 = UiIntent::ApplyFamiliarSettings {
                    patch: hw_core::familiar::FamiliarSettingsPatch::SetWorkPriority {
                        work_type: marker.0,
                        priority: next,
                    },
                };
            }
            for (marker, mut text) in &mut q_priority_text {
                text.0 = match policy.rule_for(marker.0).priority {
                    hw_core::familiar::FamiliarWorkPriority::Low => "Low",
                    hw_core::familiar::FamiliarWorkPriority::Normal => "Normal",
                    hw_core::familiar::FamiliarWorkPriority::High => "High",
                }
                .to_string();
            }
        } else {
            dialog_state.target = None;
            close_operation_dialog(&mut q_dialog);
            for mut scroll in &mut q_scroll {
                scroll.0 = Vec2::ZERO;
            }
        }
    } else {
        close_operation_dialog(&mut q_dialog);
        for mut scroll in &mut q_scroll {
            scroll.0 = Vec2::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::PendingWorldInputCapture;
    use hw_core::familiar::{FamiliarSettingsPatch, FamiliarWorkPriority, FamiliarWorkRule};
    use hw_jobs::BuildingType;

    #[derive(Resource, Default)]
    struct CollectedIntents(Vec<UiIntent>);

    fn collect_intents(
        mut reader: MessageReader<UiIntent>,
        mut collected: ResMut<CollectedIntents>,
    ) {
        collected.0.extend(reader.read().copied());
    }

    #[test]
    fn move_overlay_is_limited_to_plant_buildings() {
        let mut app = App::new();
        app.init_resource::<HoveredEntity>()
            .init_resource::<HoverActionTarget>()
            .add_systems(Update, update_move_plant_hover_target_system);
        let tank = app
            .world_mut()
            .spawn(Building {
                kind: BuildingType::Tank,
                is_provisional: false,
            })
            .id();
        let wall = app
            .world_mut()
            .spawn(Building {
                kind: BuildingType::Wall,
                is_provisional: false,
            })
            .id();
        let non_building = app.world_mut().spawn_empty().id();
        let order = app.world_mut().spawn_empty().id();
        let pending_tank = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::Tank,
                    is_provisional: false,
                },
                hw_jobs::DeconstructionPending { order },
            ))
            .id();

        for (hovered, expected) in [
            (Some(tank), Some(tank)),
            (Some(pending_tank), None),
            (Some(wall), None),
            (Some(non_building), None),
            (None, None),
        ] {
            app.world_mut().resource_mut::<HoveredEntity>().0 = hovered;
            app.update();
            assert_eq!(app.world().resource::<HoverActionTarget>().0, expected);
        }
    }

    #[test]
    fn foreground_gate_blocks_background_menu_action() {
        let mut app = App::new();
        app.add_message::<UiIntent>()
            .init_resource::<UiInputState>()
            .init_resource::<PendingWorldInputCapture>()
            .init_resource::<UiTheme>()
            .init_resource::<CollectedIntents>()
            .add_systems(Update, (ui_interaction_system, collect_intents).chain());
        let root = app.world_mut().spawn(Node::default()).id();
        let foreground = app
            .world_mut()
            .spawn((
                Interaction::Pressed,
                Button,
                MenuButton(MenuAction::ToggleDoorLock(root)),
                BackgroundColor::default(),
                ChildOf(root),
            ))
            .id();
        let background = app
            .world_mut()
            .spawn((
                Interaction::Pressed,
                Button,
                MenuButton(MenuAction::ToggleDoorLock(root)),
                BackgroundColor::default(),
            ))
            .id();
        {
            let mut state = app.world_mut().resource_mut::<UiInputState>();
            state.world_input_captured = true;
            state.foreground_capture_root = Some(root);
        }

        app.update();

        let intents = &app.world().resource::<CollectedIntents>().0;
        assert_eq!(intents.len(), 1);
        assert!(matches!(intents[0], UiIntent::ToggleDoorLock(entity) if entity == root));
        assert!(app.world().get_entity(foreground).is_ok());
        assert!(app.world().get_entity(background).is_ok());
    }

    #[test]
    fn soul_spa_cancel_button_emits_one_exact_intent_per_press() {
        let mut app = App::new();
        app.add_message::<UiIntent>()
            .init_resource::<UiInputState>()
            .init_resource::<PendingWorldInputCapture>()
            .init_resource::<UiTheme>()
            .init_resource::<CollectedIntents>()
            .add_systems(Update, (ui_interaction_system, collect_intents).chain());
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((
            Interaction::Pressed,
            Button,
            MenuButton(MenuAction::CancelSoulSpaConstruction { target }),
            BackgroundColor::default(),
        ));
        app.world_mut().spawn((
            Interaction::Hovered,
            Button,
            MenuButton(MenuAction::CancelSoulSpaConstruction { target }),
            BackgroundColor::default(),
        ));

        app.update();
        app.update();

        let intents = &app.world().resource::<CollectedIntents>().0;
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents[0],
            UiIntent::CancelSoulSpaConstruction { target: actual } if actual == target
        ));
    }

    #[test]
    fn stale_operation_target_closes_without_retargeting_and_resets_scroll() {
        let mut app = App::new();
        let stale_target = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(stale_target);
        app.init_resource::<UiNodeRegistry>()
            .init_resource::<UiTheme>()
            .insert_resource(OperationDialogState {
                target: Some(stale_target),
            })
            .add_systems(Update, update_operation_dialog_system);
        let root = app
            .world_mut()
            .spawn((
                Node {
                    display: Display::Flex,
                    ..default()
                },
                OperationDialog,
            ))
            .id();
        let scroll = app
            .world_mut()
            .spawn((ScrollPosition(Vec2::new(0.0, 240.0)), OperationDialogScroll))
            .id();

        app.update();

        assert_eq!(
            *app.world().resource::<OperationDialogState>(),
            OperationDialogState::default()
        );
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<ScrollPosition>(scroll).unwrap().0,
            Vec2::ZERO
        );
    }

    #[test]
    fn operation_dialog_binds_each_exact_target_value_and_next_action() {
        let mut app = App::new();
        app.init_resource::<UiNodeRegistry>()
            .init_resource::<UiTheme>()
            .add_systems(Update, update_operation_dialog_system);

        let familiar_name = app.world_mut().spawn(Text::new("stale name")).id();
        let threshold = app.world_mut().spawn(Text::new("stale threshold")).id();
        let max_soul = app.world_mut().spawn(Text::new("stale max")).id();
        {
            let mut nodes = app.world_mut().resource_mut::<UiNodeRegistry>();
            nodes.set_slot(UiSlot::DialogFamiliarName, familiar_name);
            nodes.set_slot(UiSlot::DialogThresholdText, threshold);
            nodes.set_slot(UiSlot::DialogMaxSoulText, max_soul);
        }

        let warning = app
            .world_mut()
            .spawn((Node::default(), OperationPolicyAllDisabledWarning))
            .id();
        let allowed_button = app
            .world_mut()
            .spawn((
                OperationPolicyAllowedButton(hw_jobs::WorkType::Chop),
                MenuButton(UiIntent::CloseDialog),
                BackgroundColor::default(),
            ))
            .id();
        let allowed_text = app
            .world_mut()
            .spawn((
                OperationPolicyAllowedText(hw_jobs::WorkType::Chop),
                Text::new("stale allowed"),
            ))
            .id();
        let priority_button = app
            .world_mut()
            .spawn((
                OperationPolicyPriorityButton(hw_jobs::WorkType::Chop),
                MenuButton(UiIntent::CloseDialog),
            ))
            .id();
        let priority_text = app
            .world_mut()
            .spawn((
                OperationPolicyPriorityText(hw_jobs::WorkType::Chop),
                Text::new("stale priority"),
            ))
            .id();

        let mut first_policy = FamiliarPolicy::default();
        first_policy.set_all_allowed(false);
        first_policy.set_rule(
            hw_jobs::WorkType::Chop,
            FamiliarWorkRule {
                allowed: false,
                priority: FamiliarWorkPriority::High,
            },
        );
        let first = app
            .world_mut()
            .spawn((
                Familiar {
                    name: "A".to_string(),
                    ..default()
                },
                FamiliarOperation {
                    fatigue_threshold: 0.7,
                    max_controlled_soul: 3,
                },
                first_policy,
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                Familiar {
                    name: "B".to_string(),
                    ..default()
                },
                FamiliarOperation {
                    fatigue_threshold: 0.0,
                    max_controlled_soul: 5,
                },
                FamiliarPolicy::default(),
            ))
            .id();

        app.insert_resource(OperationDialogState {
            target: Some(first),
        });
        app.update();

        assert_eq!(
            app.world().get::<Text>(familiar_name).unwrap().0,
            "Editing: A"
        );
        assert_eq!(app.world().get::<Text>(threshold).unwrap().0, "70%");
        assert_eq!(app.world().get::<Text>(max_soul).unwrap().0, "3");
        assert_eq!(
            app.world().get::<Node>(warning).unwrap().display,
            Display::Flex
        );
        assert_eq!(app.world().get::<Text>(allowed_text).unwrap().0, "Disabled");
        assert_eq!(app.world().get::<Text>(priority_text).unwrap().0, "High");
        assert!(matches!(
            app.world().get::<MenuButton>(allowed_button).unwrap().0,
            UiIntent::ApplyFamiliarSettings {
                patch: FamiliarSettingsPatch::SetWorkAllowed {
                    work_type: hw_jobs::WorkType::Chop,
                    allowed: true,
                },
            }
        ));
        assert!(matches!(
            app.world().get::<MenuButton>(priority_button).unwrap().0,
            UiIntent::ApplyFamiliarSettings {
                patch: FamiliarSettingsPatch::SetWorkPriority {
                    work_type: hw_jobs::WorkType::Chop,
                    priority: FamiliarWorkPriority::Low,
                },
            }
        ));
        assert_eq!(
            app.world()
                .get::<BackgroundColor>(allowed_button)
                .unwrap()
                .0,
            app.world().resource::<UiTheme>().colors.status_danger
        );

        app.world_mut()
            .resource_mut::<OperationDialogState>()
            .target = Some(second);
        app.update();

        assert_eq!(
            app.world().get::<Text>(familiar_name).unwrap().0,
            "Editing: B"
        );
        assert_eq!(
            app.world().get::<Text>(threshold).unwrap().0,
            "0% (Recruit Off)"
        );
        assert_eq!(app.world().get::<Text>(max_soul).unwrap().0, "5");
        assert_eq!(
            app.world().get::<Node>(warning).unwrap().display,
            Display::None
        );
        assert_eq!(app.world().get::<Text>(allowed_text).unwrap().0, "Enabled");
        assert_eq!(app.world().get::<Text>(priority_text).unwrap().0, "Normal");
        assert!(matches!(
            app.world().get::<MenuButton>(allowed_button).unwrap().0,
            UiIntent::ApplyFamiliarSettings {
                patch: FamiliarSettingsPatch::SetWorkAllowed {
                    work_type: hw_jobs::WorkType::Chop,
                    allowed: false,
                },
            }
        ));
        assert!(matches!(
            app.world().get::<MenuButton>(priority_button).unwrap().0,
            UiIntent::ApplyFamiliarSettings {
                patch: FamiliarSettingsPatch::SetWorkPriority {
                    work_type: hw_jobs::WorkType::Chop,
                    priority: FamiliarWorkPriority::High,
                },
            }
        ));
        assert_eq!(
            app.world()
                .get::<BackgroundColor>(allowed_button)
                .unwrap()
                .0,
            app.world().resource::<UiTheme>().colors.status_healthy
        );
    }
}
