use std::collections::HashSet;

use super::bindings::{DEFAULT_BINDINGS, InputBinding};
use super::resolver::resolve_input_chords_with_bindings;
use super::*;
use crate::test_support::minimal_app;

fn plain(key: KeyCode) -> InputChord {
    InputChord::plain(key)
}

#[test]
fn default_bindings_have_unique_exact_chords() {
    let mut chords = HashSet::new();
    for binding in DEFAULT_BINDINGS {
        assert!(
            chords.insert(binding.chord),
            "duplicate binding for {:?}",
            binding.chord
        );
    }
}

#[test]
fn every_m1_action_has_exactly_one_consumer_owner() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ConsumerOwner {
        UiIntentBridge,
        ElevationView,
    }

    fn owner(action: InputAction) -> ConsumerOwner {
        match action {
            InputAction::SaveGame | InputAction::RequestLoadGame => ConsumerOwner::UiIntentBridge,
            InputAction::CycleElevation => ConsumerOwner::ElevationView,
        }
    }

    assert_eq!(owner(InputAction::SaveGame), ConsumerOwner::UiIntentBridge);
    assert_eq!(
        owner(InputAction::RequestLoadGame),
        ConsumerOwner::UiIntentBridge
    );
    assert_eq!(
        owner(InputAction::CycleElevation),
        ConsumerOwner::ElevationView
    );
}

#[test]
fn plain_v_resolves_but_ctrl_v_does_not() {
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::KeyV)], InputContextSnapshot::default()),
        [InputAction::CycleElevation]
    );
    assert!(
        resolve_input_chords(
            &[InputChord {
                key: KeyCode::KeyV,
                modifiers: InputModifiers {
                    ctrl: true,
                    ..default()
                },
            }],
            InputContextSnapshot::default(),
        )
        .is_empty()
    );
}

#[test]
fn text_input_blocks_all_m1_actions() {
    let context = InputContextSnapshot {
        text_input_blocks_keybinds: true,
        ..default()
    };
    assert!(
        resolve_input_chords(
            &[plain(KeyCode::F5), plain(KeyCode::F9), plain(KeyCode::KeyV),],
            context,
        )
        .is_empty()
    );
}

#[test]
fn active_gesture_blocks_save_without_blocking_view_action() {
    let actions = resolve_input_chords(
        &[plain(KeyCode::F5), plain(KeyCode::KeyV)],
        InputContextSnapshot {
            has_in_progress_gesture: true,
            ..default()
        },
    );
    assert_eq!(actions, [InputAction::CycleElevation]);
}

#[test]
fn save_load_family_prefers_save_and_deduplicates_aliases() {
    let actions = resolve_input_chords(
        &[plain(KeyCode::F5), plain(KeyCode::F9)],
        InputContextSnapshot::default(),
    );
    assert_eq!(actions, [InputAction::SaveGame]);

    let alias_bindings = [
        InputBinding {
            chord: plain(KeyCode::F5),
            action: InputAction::SaveGame,
            exclusive_family: Some(InputActionFamily::SaveLoad),
            family_priority: 2,
            conflict_lane: InputConflictLane::SimulationControl,
            resolution_priority: 2,
        },
        InputBinding {
            chord: plain(KeyCode::F6),
            action: InputAction::SaveGame,
            exclusive_family: Some(InputActionFamily::SaveLoad),
            family_priority: 2,
            conflict_lane: InputConflictLane::SimulationControl,
            resolution_priority: 2,
        },
    ];
    assert_eq!(
        resolve_input_chords_with_bindings(
            &[plain(KeyCode::F5), plain(KeyCode::F6)],
            InputContextSnapshot::default(),
            &alias_bindings,
        ),
        [InputAction::SaveGame]
    );
}

#[test]
fn compatibility_is_explicit_across_conflict_lanes() {
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::F5), plain(KeyCode::KeyV)],
            InputContextSnapshot::default(),
        ),
        [InputAction::SaveGame, InputAction::CycleElevation]
    );
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::F9), plain(KeyCode::KeyV)],
            InputContextSnapshot::default(),
        ),
        [InputAction::RequestLoadGame]
    );
}

#[test]
fn left_and_right_modifiers_share_one_snapshot() {
    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::ControlRight);
    keyboard.press(KeyCode::AltLeft);
    keyboard.press(KeyCode::ShiftRight);
    keyboard.press(KeyCode::SuperLeft);

    assert_eq!(
        InputModifiers::from_keyboard(&keyboard),
        InputModifiers {
            ctrl: true,
            alt: true,
            shift: true,
            super_key: true,
        }
    );
}

#[derive(Resource, Default)]
struct OrderingTrace(Vec<&'static str>);

fn record_resolve(mut trace: ResMut<OrderingTrace>) {
    trace.0.push("resolve");
}

fn record_pointer_ingress(mut trace: ResMut<OrderingTrace>) {
    trace.0.push("pointer");
}

fn record_consume(mut trace: ResMut<OrderingTrace>) {
    trace.0.push("consume");
}

#[test]
fn m1_schedule_orders_resolve_before_pointer_ingress_and_consume() {
    let mut app = minimal_app();
    app.init_resource::<OrderingTrace>();
    configure_input_resolution_sets(&mut app);
    app.add_systems(PreUpdate, record_resolve.in_set(InputPreUpdateSet::Resolve));
    app.add_systems(
        Update,
        (
            record_pointer_ingress.in_set(InputResolutionSet::PointerIngress),
            record_consume.in_set(InputResolutionSet::Consume),
        ),
    );

    app.update();

    assert_eq!(
        app.world().resource::<OrderingTrace>().0,
        ["resolve", "pointer", "consume"]
    );
}

#[test]
fn bridge_maps_only_ui_owned_actions() {
    assert!(matches!(
        ui_intent_for_action(InputAction::SaveGame),
        Some(UiIntent::SaveGame)
    ));
    assert!(matches!(
        ui_intent_for_action(InputAction::RequestLoadGame),
        Some(UiIntent::RequestLoadGame)
    ));
    assert!(ui_intent_for_action(InputAction::CycleElevation).is_none());
}

#[test]
fn resolver_system_replaces_actions_instead_of_retaining_stale_edges() {
    let mut app = minimal_app();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<hw_ui::components::UiInputState>();
    app.init_resource::<ResolvedInputFrame>();
    app.add_systems(Update, resolve_input_frame_system);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F5);

    app.update();
    assert!(
        app.world()
            .resource::<ResolvedInputFrame>()
            .contains(InputAction::SaveGame)
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear_just_pressed(KeyCode::F5);
    app.update();
    assert!(
        app.world()
            .resource::<ResolvedInputFrame>()
            .actions()
            .is_empty()
    );
}
