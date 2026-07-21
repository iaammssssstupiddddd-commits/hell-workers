use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::input_actions::ForegroundUiGate;
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
        menu_actions::handle_pressed_action(menu_button.0, &mut ui_intent_writer);
    }
}

/// Root adapter that publishes only movable Plant buildings to the UI widget.
pub fn update_move_plant_hover_target_system(
    hovered: Res<HoveredEntity>,
    q_buildings: Query<&Building>,
    mut target: ResMut<HoverActionTarget>,
) {
    target.0 = hovered.0.filter(|entity| {
        q_buildings
            .get(*entity)
            .is_ok_and(|building| building.kind.category() == BuildingCategory::Plant)
    });
}

/// Operation Dialog のテキスト表示を更新するシステム
pub fn update_operation_dialog_system(
    selected_entity: Res<crate::interface::selection::SelectedEntity>,
    ui_nodes: Res<UiNodeRegistry>,
    q_familiars: Query<(&Familiar, &FamiliarOperation)>,
    mut q_dialog: Query<&mut Node, With<OperationDialog>>,
    mut q_text: Query<&mut Text>,
) {
    if let Some(selected) = selected_entity.0 {
        if let Ok((familiar, op)) = q_familiars.get(selected) {
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
        } else {
            close_operation_dialog(&mut q_dialog);
        }
    } else {
        close_operation_dialog(&mut q_dialog);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::PendingWorldInputCapture;
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

        for (hovered, expected) in [
            (Some(tank), Some(tank)),
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
}
