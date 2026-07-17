use std::collections::HashSet;

use hw_core::game_state::{PlayMode, TaskMode, TaskModeZoneType, TimeSpeed};
use hw_ui::components::{
    LoadConfirmDialog, MenuState, OperationDialog, SettingsPanel, UiInputState,
};

use super::bindings::{DEFAULT_BINDINGS, InputBinding};
use super::resolver::resolve_input_chords_with_bindings;
use super::*;
use crate::app_contexts::TaskContext;
use crate::entities::familiar::Familiar;
use crate::interface::selection::SelectedEntity;
use crate::test_support::minimal_app;

fn plain(key: KeyCode) -> InputChord {
    InputChord::plain(key)
}

fn modified(key: KeyCode, modifiers: InputModifiers) -> InputChord {
    InputChord { key, modifiers }
}

fn ctrl(key: KeyCode) -> InputChord {
    modified(
        key,
        InputModifiers {
            ctrl: true,
            ..default()
        },
    )
}

fn area_edit_context() -> InputContextSnapshot {
    InputContextSnapshot {
        play_mode: PlayMode::TaskDesignation,
        task_mode: TaskMode::AreaSelection(None),
        ..default()
    }
}

fn familiar_context() -> InputContextSnapshot {
    InputContextSnapshot {
        has_selected_familiar: true,
        ..default()
    }
}

fn active_context(play_mode: PlayMode) -> InputContextSnapshot {
    InputContextSnapshot {
        play_mode,
        has_selected_familiar: true,
        ..default()
    }
}

fn paused_context() -> InputContextSnapshot {
    InputContextSnapshot {
        top_overlay: Some(InputOverlay::Pause),
        simulation_paused: true,
        logic_shortcuts_enabled: false,
        has_selected_familiar: true,
        ..default()
    }
}

#[test]
fn default_bindings_have_unique_contextual_claims() {
    let mut claims = HashSet::new();
    for binding in DEFAULT_BINDINGS {
        assert!(
            claims.insert((binding.chord, binding.context)),
            "duplicate binding claim for {:?} in {:?}",
            binding.chord,
            binding.context,
        );
    }
}

#[test]
fn every_action_has_exactly_one_consumer_owner() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ConsumerOwner {
        UiIntentBridge,
        ElevationView,
        RenderDebugToggle,
        DebugSpawn,
        FamiliarCommand,
        ActiveModeCancel,
        AreaEdit,
        ListNavigation,
    }

    fn owner(action: InputAction) -> ConsumerOwner {
        match action {
            InputAction::SaveGame
            | InputAction::RequestLoadGame
            | InputAction::ToggleArchitect
            | InputAction::ToggleZones
            | InputAction::TogglePause
            | InputAction::TimePaused
            | InputAction::TimeNormal
            | InputAction::TimeFast
            | InputAction::TimeSuper
            | InputAction::CancelLoadConfirm
            | InputAction::CloseSettings
            | InputAction::CloseOperationDialog => ConsumerOwner::UiIntentBridge,
            InputAction::CycleElevation => ConsumerOwner::ElevationView,
            InputAction::ToggleRender3d
            | InputAction::CycleRttQuality
            | InputAction::ToggleRttDirectionalLight
            | InputAction::ToggleRttTerrain
            | InputAction::ToggleRttSceneObjects
            | InputAction::ToggleDebug => ConsumerOwner::RenderDebugToggle,
            InputAction::DebugSpawnSoul | InputAction::DebugSpawnFamiliar => {
                ConsumerOwner::DebugSpawn
            }
            InputAction::FamiliarChop
            | InputAction::FamiliarMine
            | InputAction::FamiliarHaul
            | InputAction::FamiliarBuild
            | InputAction::FamiliarCancelDesignation
            | InputAction::ToggleFamiliarIdlePatrol => ConsumerOwner::FamiliarCommand,
            InputAction::CancelActiveMode | InputAction::CloseOpenMenu => {
                ConsumerOwner::ActiveModeCancel
            }
            InputAction::AreaCopy
            | InputAction::AreaPaste
            | InputAction::AreaUndo
            | InputAction::AreaRedo
            | InputAction::AreaSavePreset1
            | InputAction::AreaSavePreset2
            | InputAction::AreaSavePreset3
            | InputAction::AreaLoadPreset1
            | InputAction::AreaLoadPreset2
            | InputAction::AreaLoadPreset3 => ConsumerOwner::AreaEdit,
            InputAction::ListNext | InputAction::ListPrevious => ConsumerOwner::ListNavigation,
        }
    }

    for action in [
        InputAction::SaveGame,
        InputAction::RequestLoadGame,
        InputAction::CycleElevation,
        InputAction::ToggleRender3d,
        InputAction::CycleRttQuality,
        InputAction::ToggleRttDirectionalLight,
        InputAction::ToggleRttTerrain,
        InputAction::ToggleRttSceneObjects,
        InputAction::ToggleDebug,
        InputAction::DebugSpawnSoul,
        InputAction::DebugSpawnFamiliar,
        InputAction::ToggleArchitect,
        InputAction::ToggleZones,
        InputAction::TogglePause,
        InputAction::TimePaused,
        InputAction::TimeNormal,
        InputAction::TimeFast,
        InputAction::TimeSuper,
        InputAction::FamiliarChop,
        InputAction::FamiliarMine,
        InputAction::FamiliarHaul,
        InputAction::FamiliarBuild,
        InputAction::FamiliarCancelDesignation,
        InputAction::ToggleFamiliarIdlePatrol,
        InputAction::CancelLoadConfirm,
        InputAction::CloseSettings,
        InputAction::CloseOperationDialog,
        InputAction::CancelActiveMode,
        InputAction::CloseOpenMenu,
        InputAction::AreaCopy,
        InputAction::AreaPaste,
        InputAction::AreaUndo,
        InputAction::AreaRedo,
        InputAction::AreaSavePreset1,
        InputAction::AreaSavePreset2,
        InputAction::AreaSavePreset3,
        InputAction::AreaLoadPreset1,
        InputAction::AreaLoadPreset2,
        InputAction::AreaLoadPreset3,
        InputAction::ListNext,
        InputAction::ListPrevious,
    ] {
        let _ = owner(action);
    }
}

#[test]
fn exact_plain_chords_do_not_accept_modifiers() {
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::KeyV)], InputContextSnapshot::default()),
        [InputAction::CycleElevation]
    );
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::KeyZ)], InputContextSnapshot::default()),
        [InputAction::ToggleZones]
    );
    for key in [KeyCode::KeyV, KeyCode::KeyZ, KeyCode::KeyC] {
        assert!(
            resolve_input_chords(
                &[InputChord {
                    key,
                    modifiers: InputModifiers {
                        ctrl: true,
                        ..default()
                    },
                }],
                familiar_context(),
            )
            .is_empty()
        );
    }
}

#[test]
fn area_edit_exact_chords_do_not_fall_through_to_world_actions() {
    assert_eq!(
        resolve_input_chords(&[ctrl(KeyCode::KeyV)], area_edit_context()),
        [InputAction::AreaPaste]
    );
    assert_eq!(
        resolve_input_chords(&[ctrl(KeyCode::KeyZ)], area_edit_context()),
        [InputAction::AreaUndo]
    );
    assert_eq!(
        resolve_input_chords(
            &[
                modified(
                    KeyCode::KeyZ,
                    InputModifiers {
                        ctrl: true,
                        shift: true,
                        ..default()
                    },
                ),
                plain(KeyCode::KeyB),
            ],
            area_edit_context(),
        ),
        [InputAction::AreaRedo]
    );
}

#[test]
fn area_edit_preset_family_preserves_slot_and_operation_priority() {
    assert_eq!(
        resolve_input_chords(
            &[ctrl(KeyCode::Digit1), ctrl(KeyCode::Digit2)],
            area_edit_context(),
        ),
        [InputAction::AreaSavePreset1]
    );
    assert_eq!(
        resolve_input_chords(
            &[
                ctrl(KeyCode::KeyC),
                modified(
                    KeyCode::Digit3,
                    InputModifiers {
                        alt: true,
                        ..default()
                    },
                ),
            ],
            area_edit_context(),
        ),
        [InputAction::AreaLoadPreset3]
    );
}

#[test]
fn area_edit_requires_current_task_designation_state() {
    assert!(
        resolve_input_chords(
            &[ctrl(KeyCode::KeyV)],
            InputContextSnapshot {
                task_mode: TaskMode::AreaSelection(None),
                pending_play_mode: Some(PlayMode::TaskDesignation),
                ..default()
            },
        )
        .is_empty()
    );
    assert!(
        resolve_input_chords(
            &[ctrl(KeyCode::KeyV)],
            InputContextSnapshot {
                play_mode: PlayMode::TaskDesignation,
                task_mode: TaskMode::DesignateChop(None),
                ..default()
            },
        )
        .is_empty()
    );
    assert!(
        resolve_input_chords(
            &[ctrl(KeyCode::KeyV)],
            InputContextSnapshot {
                logic_shortcuts_enabled: false,
                ..area_edit_context()
            },
        )
        .is_empty()
    );
}

#[test]
fn list_navigation_is_exact_and_world_normal_only() {
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::Tab)], InputContextSnapshot::default()),
        [InputAction::ListNext]
    );
    assert_eq!(
        resolve_input_chords(
            &[modified(
                KeyCode::Tab,
                InputModifiers {
                    shift: true,
                    ..default()
                },
            )],
            InputContextSnapshot::default(),
        ),
        [InputAction::ListPrevious]
    );
    assert!(resolve_input_chords(&[plain(KeyCode::Tab)], area_edit_context()).is_empty());
}

#[test]
fn debug_spawn_uses_resolver_time_visibility_snapshot() {
    assert!(
        resolve_input_chords(
            &[plain(KeyCode::KeyP), plain(KeyCode::KeyO)],
            InputContextSnapshot::default(),
        )
        .is_empty()
    );
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::F12), plain(KeyCode::KeyP)],
            InputContextSnapshot {
                debug_visible: true,
                ..default()
            },
        ),
        [InputAction::ToggleDebug, InputAction::DebugSpawnSoul]
    );
}

#[test]
fn migrated_debug_actions_are_modal_suppressed_and_world_compatible() {
    let actions = resolve_input_chords(
        &[plain(KeyCode::KeyB), plain(KeyCode::F3)],
        InputContextSnapshot::default(),
    );
    assert_eq!(
        actions,
        [InputAction::ToggleArchitect, InputAction::ToggleRender3d]
    );
    assert!(
        resolve_input_chords(
            &[plain(KeyCode::F3), plain(KeyCode::Tab)],
            InputContextSnapshot {
                top_overlay: Some(InputOverlay::Settings),
                ..default()
            },
        )
        .is_empty()
    );
    assert!(resolve_input_chords(&[plain(KeyCode::F3)], paused_context()).is_empty());

    let text_context = InputContextSnapshot {
        text_input_blocks_keybinds: true,
        debug_visible: true,
        ..area_edit_context()
    };
    assert!(
        resolve_input_chords(
            &[
                ctrl(KeyCode::KeyV),
                plain(KeyCode::Tab),
                plain(KeyCode::F12),
                plain(KeyCode::KeyP),
            ],
            text_context,
        )
        .is_empty()
    );
}

#[test]
fn text_input_blocks_shortcuts_without_an_overlay() {
    let context = InputContextSnapshot {
        text_input_blocks_keybinds: true,
        ..default()
    };
    assert!(
        resolve_input_chords(
            &[
                plain(KeyCode::F5),
                plain(KeyCode::KeyB),
                plain(KeyCode::Space),
            ],
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

    let first = DEFAULT_BINDINGS[0];
    let alias_bindings = [
        first,
        InputBinding {
            chord: plain(KeyCode::F6),
            ..first
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
fn world_and_familiar_claims_resolve_to_one_semantic_action_per_chord() {
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::Digit1)], InputContextSnapshot::default()),
        [InputAction::TimePaused]
    );
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::Digit1)], familiar_context()),
        [InputAction::FamiliarChop]
    );
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::KeyB)], familiar_context()),
        [InputAction::FamiliarBuild]
    );
}

#[test]
fn familiar_family_preserves_legacy_else_if_priority() {
    let actions = resolve_input_chords(
        &[
            plain(KeyCode::KeyC),
            plain(KeyCode::KeyM),
            plain(KeyCode::Escape),
        ],
        familiar_context(),
    );
    assert_eq!(actions, [InputAction::FamiliarChop]);
}

#[test]
fn familiar_command_beats_world_menu_across_chords() {
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::KeyC), plain(KeyCode::KeyZ)],
            familiar_context(),
        ),
        [InputAction::FamiliarChop]
    );
}

#[test]
fn active_modes_block_world_and_familiar_mode_shortcuts() {
    let context = active_context(PlayMode::FloorPlace);
    assert_eq!(
        resolve_input_chords(
            &[
                plain(KeyCode::KeyB),
                plain(KeyCode::KeyC),
                plain(KeyCode::Digit2),
            ],
            context,
        ),
        [InputAction::TimeNormal]
    );
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::Escape)],
            active_context(PlayMode::BuildingMove),
        ),
        [InputAction::CancelActiveMode]
    );
}

#[test]
fn every_task_mode_owner_claims_escape_before_the_familiar_toggle() {
    for task_mode in [
        TaskMode::DesignateChop(None),
        TaskMode::DesignateChop(Some(Vec2::ZERO)),
        TaskMode::DesignateMine(None),
        TaskMode::DesignateHaul(None),
        TaskMode::CancelDesignation(None),
        TaskMode::SelectBuildTarget,
        TaskMode::AreaSelection(None),
        TaskMode::AssignTask(None),
        TaskMode::ZonePlacement(TaskModeZoneType::Stockpile, None),
        TaskMode::ZoneRemoval(TaskModeZoneType::Yard, None),
        TaskMode::FloorPlace(None),
        TaskMode::WallPlace(None),
        TaskMode::DreamPlanting(None),
        TaskMode::SoulSpaPlace(None),
    ] {
        assert_eq!(
            resolve_input_chords(
                &[plain(KeyCode::Escape)],
                InputContextSnapshot {
                    task_mode,
                    has_selected_familiar: true,
                    ..default()
                },
            ),
            [InputAction::CancelActiveMode],
            "task mode {task_mode:?} must own Escape",
        );
    }
}

#[test]
fn pending_non_normal_mode_blocks_familiar_and_world_shortcuts() {
    let context = InputContextSnapshot {
        has_selected_familiar: true,
        pending_play_mode: Some(PlayMode::BuildingPlace),
        ..default()
    };
    assert!(resolve_input_chords(&[plain(KeyCode::KeyC)], context.clone()).is_empty());
    assert!(resolve_input_chords(&[plain(KeyCode::KeyB)], context.clone()).is_empty());
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::Escape)], context),
        [InputAction::CancelActiveMode]
    );
}

#[test]
fn open_menu_escape_beats_familiar_escape() {
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::Escape)],
            InputContextSnapshot {
                menu_state: MenuState::Architect,
                has_selected_familiar: true,
                ..default()
            },
        ),
        [InputAction::CloseOpenMenu]
    );
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::Escape)],
            InputContextSnapshot {
                play_mode: PlayMode::BuildingPlace,
                menu_state: MenuState::Architect,
                has_selected_familiar: true,
                ..default()
            },
        ),
        [InputAction::CancelActiveMode]
    );
}

#[test]
fn pause_whitelist_and_time_priority_are_deterministic() {
    assert!(resolve_input_chords(&[plain(KeyCode::KeyC)], paused_context()).is_empty());
    assert_eq!(
        resolve_input_chords(&[plain(KeyCode::Escape)], paused_context()),
        [InputAction::TogglePause]
    );
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::Escape), plain(KeyCode::Digit4)],
            paused_context(),
        ),
        [InputAction::TimeSuper]
    );
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::Space), plain(KeyCode::F5)],
            paused_context(),
        ),
        [InputAction::TogglePause]
    );
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::Digit2), plain(KeyCode::F5)],
            paused_context(),
        ),
        [InputAction::TimeNormal, InputAction::SaveGame]
    );
}

#[test]
fn modal_overlay_owns_escape_even_with_stale_text_focus() {
    for (overlay, expected) in [
        (InputOverlay::LoadConfirm, InputAction::CancelLoadConfirm),
        (InputOverlay::Settings, InputAction::CloseSettings),
        (
            InputOverlay::OperationDialog,
            InputAction::CloseOperationDialog,
        ),
    ] {
        let context = InputContextSnapshot {
            text_input_blocks_keybinds: true,
            top_overlay: Some(overlay),
            ..default()
        };
        assert_eq!(
            resolve_input_chords(&[plain(KeyCode::Escape)], context.clone()),
            [expected]
        );
        assert!(resolve_input_chords(&[plain(KeyCode::F5)], context).is_empty());
    }
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
fn in_progress_gesture_denies_mode_and_time_coexistence() {
    assert_eq!(
        resolve_input_chords(
            &[plain(KeyCode::Escape), plain(KeyCode::Digit2)],
            InputContextSnapshot {
                play_mode: PlayMode::TaskDesignation,
                task_mode: TaskMode::DesignateChop(Some(Vec2::ZERO)),
                has_in_progress_gesture: true,
                ..default()
            },
        ),
        [InputAction::CancelActiveMode]
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
fn m2_schedule_orders_resolve_before_pointer_ingress_and_consume() {
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
fn bridge_maps_ui_owned_actions() {
    assert!(matches!(
        ui_intent_for_action(InputAction::SaveGame),
        Some(UiIntent::SaveGame)
    ));
    assert!(matches!(
        ui_intent_for_action(InputAction::ToggleArchitect),
        Some(UiIntent::ToggleArchitect)
    ));
    assert!(matches!(
        ui_intent_for_action(InputAction::TimeFast),
        Some(UiIntent::SetTimeSpeed(TimeSpeed::Fast))
    ));
    assert!(matches!(
        ui_intent_for_action(InputAction::CloseOperationDialog),
        Some(UiIntent::CloseDialog)
    ));
    assert!(ui_intent_for_action(InputAction::FamiliarChop).is_none());
    assert!(ui_intent_for_action(InputAction::CancelActiveMode).is_none());
}

fn resolver_app() -> App {
    let mut app = minimal_app();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<UiInputState>()
        .init_resource::<ResolvedInputFrame>()
        .init_resource::<TaskContext>()
        .init_resource::<MenuState>()
        .init_resource::<SelectedEntity>()
        .init_resource::<crate::DebugVisible>()
        .init_resource::<Time<Virtual>>()
        .insert_resource(State::new(PlayMode::Normal))
        .init_resource::<NextState<PlayMode>>();
    configure_input_resolution_sets(&mut app);
    app.add_systems(
        PreUpdate,
        resolve_input_frame_system.in_set(InputPreUpdateSet::Resolve),
    );
    app
}

#[test]
fn resolver_system_replaces_actions_instead_of_retaining_stale_edges() {
    let mut app = resolver_app();
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
    let frame = app.world().resource::<ResolvedInputFrame>();
    assert!(frame.actions().is_empty());
    assert!(!frame.pointer_selection_suppressed());
}

#[test]
fn resolver_frame_keeps_the_frame_start_familiar_target_and_suppresses_clicks() {
    let mut app = resolver_app();
    let familiar = app.world_mut().spawn(Familiar::default()).id();
    app.world_mut().resource_mut::<SelectedEntity>().0 = Some(familiar);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);

    app.update();

    let frame = app.world().resource::<ResolvedInputFrame>();
    assert_eq!(frame.selected_familiar(), Some(familiar));
    assert_eq!(frame.actions(), [InputAction::FamiliarChop]);
    assert!(frame.pointer_selection_suppressed());
}

#[test]
fn redundant_pending_current_mode_does_not_block_shortcuts() {
    let mut app = resolver_app();
    let familiar = app.world_mut().spawn(Familiar::default()).id();
    app.world_mut().resource_mut::<SelectedEntity>().0 = Some(familiar);
    app.world_mut()
        .resource_mut::<NextState<PlayMode>>()
        .set(PlayMode::Normal);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);

    app.update();

    assert_eq!(
        app.world().resource::<ResolvedInputFrame>().actions(),
        [InputAction::FamiliarChop]
    );
}

#[test]
fn time_action_suppresses_same_frame_selection_ingress() {
    let mut app = resolver_app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit2);

    app.update();

    let frame = app.world().resource::<ResolvedInputFrame>();
    assert_eq!(frame.actions(), [InputAction::TimeNormal]);
    assert!(frame.pointer_selection_suppressed());
}

#[test]
fn task_mode_drag_blocks_save_without_an_area_edit_session() {
    let mut app = resolver_app();
    app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::DreamPlanting(Some(Vec2::ZERO));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F5);

    app.update();

    assert!(
        app.world()
            .resource::<ResolvedInputFrame>()
            .actions()
            .is_empty()
    );
}

#[test]
fn paused_familiar_edge_does_not_fire_after_unpause() {
    let mut app = resolver_app();
    let familiar = app.world_mut().spawn(Familiar::default()).id();
    app.world_mut().resource_mut::<SelectedEntity>().0 = Some(familiar);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);

    app.update();
    assert!(
        app.world()
            .resource::<ResolvedInputFrame>()
            .actions()
            .is_empty()
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear_just_pressed(KeyCode::KeyC);
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    app.update();

    assert!(
        app.world()
            .resource::<ResolvedInputFrame>()
            .actions()
            .is_empty()
    );
}

fn resolve_escape_with_overlays(
    load_confirm: Display,
    settings: Display,
    paused: bool,
    operation: Display,
) -> Vec<InputAction> {
    let mut app = resolver_app();
    app.world_mut().spawn((
        Node {
            display: operation,
            ..default()
        },
        OperationDialog,
    ));
    app.world_mut().spawn((
        Node {
            display: settings,
            ..default()
        },
        SettingsPanel,
    ));
    app.world_mut().spawn((
        Node {
            display: load_confirm,
            ..default()
        },
        LoadConfirmDialog,
    ));
    if paused {
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);

    app.update();

    app.world()
        .resource::<ResolvedInputFrame>()
        .actions()
        .to_vec()
}

#[test]
fn resolver_context_uses_visual_overlay_stack_order_and_ignores_hidden_nodes() {
    assert_eq!(
        resolve_escape_with_overlays(Display::Flex, Display::Flex, true, Display::Flex),
        [InputAction::CancelLoadConfirm]
    );
    assert_eq!(
        resolve_escape_with_overlays(Display::None, Display::Flex, true, Display::Flex),
        [InputAction::CloseSettings]
    );
    assert_eq!(
        resolve_escape_with_overlays(Display::None, Display::None, true, Display::Flex),
        [InputAction::TogglePause]
    );
    assert_eq!(
        resolve_escape_with_overlays(Display::None, Display::None, false, Display::Flex),
        [InputAction::CloseOperationDialog]
    );
}

#[test]
fn in_progress_familiar_task_mode_blocks_familiar_shortcuts() {
    let context = InputContextSnapshot {
        has_selected_familiar: true,
        task_mode: TaskMode::DesignateChop(Some(Vec2::ZERO)),
        ..default()
    };
    assert!(resolve_input_chords(&[plain(KeyCode::KeyM)], context).is_empty());
}
