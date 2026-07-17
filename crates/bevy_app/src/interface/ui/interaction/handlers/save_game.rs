use bevy::prelude::*;
use hw_ui::UiIntent;
use hw_ui::interaction::dialog::{close_load_confirm_dialog, open_load_confirm_dialog};

use super::super::intent_context::IntentUiQueries;
use super::begin_overlay_open;
use crate::systems::save::SaveLoadState;

pub(crate) fn handle(intent: UiIntent, ui: &mut IntentUiQueries) {
    match intent {
        UiIntent::SaveGame => {
            if *ui.save_load_state == SaveLoadState::Idle {
                *ui.save_load_state = SaveLoadState::SaveRequested;
                info!("Save requested");
            }
        }
        UiIntent::RequestLoadGame => {
            if !ui.save_path.as_path().exists() {
                warn!("No save file at {}", ui.save_path.as_path().display());
                return;
            }
            begin_overlay_open(&mut ui.input_focus);
            open_load_confirm_dialog(&mut ui.q_load_confirm);
        }
        UiIntent::ConfirmLoadGame => {
            close_load_confirm_dialog(&mut ui.q_load_confirm);
            if *ui.save_load_state == SaveLoadState::Idle {
                *ui.save_load_state = SaveLoadState::LoadRequested;
                info!("Load requested from confirmation dialog");
            }
        }
        UiIntent::CancelLoadConfirm => {
            close_load_confirm_dialog(&mut ui.q_load_confirm);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::input_actions::{
        InputAction, InputModifiers, InputResolutionSet, ResolvedInputFrame,
        configure_input_resolution_sets, input_action_to_ui_intent_system,
    };
    use crate::systems::GameSystemSet;
    use crate::systems::save::SavePath;
    use crate::test_support::minimal_app;
    use bevy::input_focus::InputFocus;
    use hw_ui::components::LoadConfirmDialog;

    fn request_save(mut ui: IntentUiQueries) {
        handle(UiIntent::SaveGame, &mut ui);
    }

    fn request_load(mut ui: IntentUiQueries) {
        handle(UiIntent::RequestLoadGame, &mut ui);
    }

    fn confirm_load(mut ui: IntentUiQueries) {
        handle(UiIntent::ConfirmLoadGame, &mut ui);
    }

    fn handle_save_intents(mut intents: MessageReader<UiIntent>, mut ui: IntentUiQueries) {
        for intent in intents.read().copied() {
            handle(intent, &mut ui);
        }
    }

    fn unique_save_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "hell-workers-{label}-{}-{nonce}.ron",
            std::process::id()
        ))
    }

    fn app_with_load_dialog(path: PathBuf, display: Display) -> (App, Entity) {
        let mut app = minimal_app();
        app.init_resource::<SaveLoadState>();
        app.insert_resource(InputFocus::from_entity(Entity::PLACEHOLDER));
        app.insert_resource(SavePath::new(path));
        let dialog = app
            .world_mut()
            .spawn((
                Node {
                    display,
                    ..default()
                },
                LoadConfirmDialog,
            ))
            .id();
        (app, dialog)
    }

    #[test]
    fn save_intent_requests_save_when_idle() {
        let (mut app, _) = app_with_load_dialog(unique_save_path("save-request"), Display::None);
        app.add_systems(Update, request_save);

        app.update();

        assert_eq!(
            *app.world().resource::<SaveLoadState>(),
            SaveLoadState::SaveRequested
        );
    }

    #[test]
    fn load_request_without_save_is_a_noop() {
        let (mut app, dialog) =
            app_with_load_dialog(unique_save_path("missing-load"), Display::None);
        app.add_systems(Update, request_load);

        app.update();

        assert_eq!(
            *app.world().resource::<SaveLoadState>(),
            SaveLoadState::Idle
        );
        assert_eq!(
            app.world().entity(dialog).get::<Node>().unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(Entity::PLACEHOLDER)
        );
    }

    #[test]
    fn existing_save_opens_confirmation_before_requesting_load() {
        let path = unique_save_path("confirm-load");
        std::fs::write(&path, b"test save placeholder").unwrap();
        let (mut app, dialog) = app_with_load_dialog(path.clone(), Display::None);
        app.add_systems(Update, request_load);

        app.update();

        assert_eq!(
            *app.world().resource::<SaveLoadState>(),
            SaveLoadState::Idle
        );
        assert_eq!(
            app.world().entity(dialog).get::<Node>().unwrap().display,
            Display::Flex
        );
        assert!(app.world().resource::<InputFocus>().get().is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn confirm_is_the_only_load_intent_that_requests_loading() {
        let (mut app, dialog) =
            app_with_load_dialog(unique_save_path("confirmed-load"), Display::Flex);
        app.add_systems(Update, confirm_load);

        app.update();

        assert_eq!(
            *app.world().resource::<SaveLoadState>(),
            SaveLoadState::LoadRequested
        );
        assert_eq!(
            app.world().entity(dialog).get::<Node>().unwrap().display,
            Display::None
        );
    }

    #[test]
    fn resolver_bridge_reaches_save_handler_in_the_same_update() {
        let path = unique_save_path("same-frame-load-intent");
        std::fs::write(&path, b"test save placeholder").unwrap();
        let (mut app, dialog) = app_with_load_dialog(path.clone(), Display::None);
        app.add_message::<UiIntent>();
        app.init_resource::<ResolvedInputFrame>();
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::RequestLoadGame],
                None,
                true,
            );
        app.configure_sets(
            Update,
            (GameSystemSet::Input, GameSystemSet::Interface).chain(),
        );
        configure_input_resolution_sets(&mut app);
        app.add_systems(
            Update,
            input_action_to_ui_intent_system.in_set(InputResolutionSet::Consume),
        );
        app.add_systems(Update, handle_save_intents.in_set(GameSystemSet::Interface));

        app.update();

        assert_eq!(
            *app.world().resource::<SaveLoadState>(),
            SaveLoadState::Idle
        );
        assert_eq!(
            app.world().entity(dialog).get::<Node>().unwrap().display,
            Display::Flex
        );
        std::fs::remove_file(path).unwrap();
    }
}
