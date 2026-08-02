use bevy::prelude::*;
use hw_ui::components::OperationDialogScroll;
use hw_ui::help::{HelpNavigationScrollArea, HelpPanelContent, HelpPanelState, HelpScrollArea};

use crate::input_actions::{InputOverlay, PendingWorldInputCapture};

pub(crate) type HelpScrollAreas<'w, 's> = Query<
    'w,
    's,
    &'static mut ScrollPosition,
    (
        Or<(With<HelpScrollArea>, With<HelpNavigationScrollArea>)>,
        Without<OperationDialogScroll>,
    ),
>;

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HelpPauseGuard {
    paused_by_help: bool,
}

pub(crate) fn apply_accepted_help_open_system(
    pending: Res<PendingWorldInputCapture>,
    mut settings_requests: MessageReader<hw_familiar_ai::FamiliarSettingsChangeRequest>,
    content: Option<Res<HelpPanelContent>>,
    state: Option<ResMut<HelpPanelState>>,
    guard: Option<ResMut<HelpPauseGuard>>,
    mut time: ResMut<Time<Virtual>>,
    mut scroll_areas: HelpScrollAreas,
) {
    let has_unread_settings_request = settings_requests.read().count() > 0;
    if !pending.accepts_overlay(InputOverlay::Help) {
        return;
    }
    if has_unread_settings_request {
        return;
    }
    let (Some(content), Some(mut state), Some(mut guard)) = (content, state, guard) else {
        return;
    };
    if state.open {
        return;
    }

    open_help(
        &content,
        &mut state,
        &mut guard,
        &mut time,
        &mut scroll_areas,
    );
}

pub(crate) fn handle_help_intent(
    intent: hw_ui::UiIntent,
    pending: &PendingWorldInputCapture,
    content: &HelpPanelContent,
    state: &mut HelpPanelState,
    guard: &mut HelpPauseGuard,
    time: &mut Time<Virtual>,
    scroll_areas: &mut HelpScrollAreas<'_, '_>,
) {
    match intent {
        hw_ui::UiIntent::OpenHelp { opener }
            if pending.accepts(InputOverlay::Help, opener) && !state.open =>
        {
            open_help(content, state, guard, time, scroll_areas);
        }
        hw_ui::UiIntent::OpenHelp { .. } => {}
        hw_ui::UiIntent::CloseHelp if state.open => {
            close_help(state, guard, time);
        }
        hw_ui::UiIntent::CloseHelp => {}
        _ => {}
    }
}

fn close_help(state: &mut HelpPanelState, guard: &mut HelpPauseGuard, time: &mut Time<Virtual>) {
    state.close();
    if guard.paused_by_help {
        time.unpause();
    }
    *guard = HelpPauseGuard::default();
}

fn open_help(
    content: &HelpPanelContent,
    state: &mut HelpPanelState,
    guard: &mut HelpPauseGuard,
    time: &mut Time<Virtual>,
    scroll_areas: &mut HelpScrollAreas<'_, '_>,
) {
    let Some(first_topic) = content.first_topic_id() else {
        return;
    };

    state.open_at(first_topic);
    for mut position in scroll_areas.iter_mut() {
        position.0 = Vec2::ZERO;
    }

    guard.paused_by_help = !time.is_paused();
    if guard.paused_by_help {
        time.pause();
    }
}

pub(crate) fn reset_root_help_state(world: &mut World) {
    let paused_by_help = world
        .get_resource::<HelpPauseGuard>()
        .is_some_and(|guard| guard.paused_by_help);

    if paused_by_help {
        if let Some(mut time) = world.get_resource_mut::<Time<Virtual>>() {
            time.unpause();
        }
        let mut pause_roots =
            world.query_filtered::<&mut Node, With<hw_ui::components::PauseMenu>>();
        for mut node in pause_roots.iter_mut(world) {
            node.display = Display::None;
        }
    }

    if world.contains_resource::<HelpPauseGuard>() {
        world.insert_resource(HelpPauseGuard::default());
    }
    if world.contains_resource::<PendingWorldInputCapture>() {
        world.insert_resource(PendingWorldInputCapture::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input_focus::InputFocus;
    use hw_ui::components::{MenuState, UiInputCapture, UiInputState};
    use hw_ui::help::{HelpSection, HelpSectionId, HelpTopic, HelpTopicId};

    use crate::input_actions::{
        InputAction, InputModifiers, ResolvedInputFrame,
        request_capture_from_resolved_actions_system, reset_pending_world_input_capture_system,
    };
    use crate::interface::selection::SelectedEntity;
    use crate::systems::save::SavePath;
    use crate::test_support::minimal_app;

    fn content() -> HelpPanelContent {
        HelpPanelContent::new([HelpSection::new(
            HelpSectionId::new("section"),
            "Section",
            [HelpTopic::new(HelpTopicId::new("topic"), "Topic", [])],
        )])
    }

    #[test]
    fn close_unpauses_only_when_help_owned_the_pause() {
        let mut world = World::new();
        world.insert_resource(Time::<Virtual>::default());
        world.insert_resource(HelpPauseGuard {
            paused_by_help: true,
        });
        world.resource_mut::<Time<Virtual>>().pause();
        let pause_root = world
            .spawn((
                Node {
                    display: Display::Flex,
                    ..default()
                },
                hw_ui::components::PauseMenu,
            ))
            .id();

        reset_root_help_state(&mut world);
        assert!(!world.resource::<Time<Virtual>>().is_paused());
        assert_eq!(
            world.entity(pause_root).get::<Node>().unwrap().display,
            Display::None
        );
        assert_eq!(
            *world.resource::<HelpPauseGuard>(),
            HelpPauseGuard::default()
        );

        world.resource_mut::<Time<Virtual>>().pause();
        world
            .entity_mut(pause_root)
            .get_mut::<Node>()
            .unwrap()
            .display = Display::Flex;
        reset_root_help_state(&mut world);
        assert!(world.resource::<Time<Virtual>>().is_paused());
        assert_eq!(
            world.entity(pause_root).get::<Node>().unwrap().display,
            Display::Flex
        );
    }

    #[test]
    fn close_matrix_preserves_preexisting_pause() {
        let mut state = HelpPanelState {
            open: true,
            active_topic: Some(HelpTopicId::new("topic")),
        };
        let mut time = Time::<Virtual>::default();
        time.pause();
        let mut guard = HelpPauseGuard {
            paused_by_help: false,
        };

        close_help(&mut state, &mut guard, &mut time);
        assert!(!state.open);
        assert!(time.is_paused());

        state.open = true;
        guard.paused_by_help = true;
        close_help(&mut state, &mut guard, &mut time);
        assert!(!time.is_paused());
    }

    #[test]
    fn catalog_fixture_is_non_empty() {
        assert_eq!(content().first_topic_id(), Some(HelpTopicId::new("topic")));
    }

    fn open_test_app(paused: bool) -> App {
        let mut app = minimal_app();
        app.init_resource::<PendingWorldInputCapture>()
            .add_message::<hw_familiar_ai::FamiliarSettingsChangeRequest>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<UiInputState>()
            .init_resource::<MenuState>()
            .init_resource::<HelpPanelState>()
            .init_resource::<HelpPauseGuard>()
            .init_resource::<SelectedEntity>()
            .init_resource::<Time<Virtual>>()
            .insert_resource(content())
            .insert_resource(SavePath::new(
                "/definitely/missing/help-controller-test-save.ron",
            ))
            .insert_resource(InputFocus::from_entity(Entity::PLACEHOLDER))
            .add_systems(
                Update,
                (
                    request_capture_from_resolved_actions_system,
                    apply_accepted_help_open_system,
                )
                    .chain(),
            );
        app.world_mut().spawn((
            Node {
                display: Display::None,
                ..default()
            },
            UiInputCapture,
            hw_ui::help::HelpPanel,
        ));
        app.world_mut().spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, 80.0)),
            HelpScrollArea,
        ));
        app.world_mut().spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, 40.0)),
            HelpNavigationScrollArea,
        ));
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::OpenHelp],
                None,
                false,
            );
        if paused {
            app.world_mut().resource_mut::<Time<Virtual>>().pause();
        }
        app
    }

    #[test]
    fn accepted_open_pauses_before_simulation_and_resets_the_topic() {
        let mut app = open_test_app(false);
        app.update();

        let state = app.world().resource::<HelpPanelState>();
        assert!(state.open);
        assert_eq!(state.active_topic, Some(HelpTopicId::new("topic")));
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert!(app.world().resource::<HelpPauseGuard>().paused_by_help);
        let position = app
            .world_mut()
            .query_filtered::<&ScrollPosition, With<HelpScrollArea>>()
            .single(app.world())
            .unwrap();
        assert_eq!(position.0, Vec2::ZERO);
        let navigation_position = app
            .world_mut()
            .query_filtered::<&ScrollPosition, With<HelpNavigationScrollArea>>()
            .single(app.world())
            .unwrap();
        assert_eq!(navigation_position.0, Vec2::ZERO);
    }

    #[test]
    fn opening_from_pause_does_not_claim_pause_ownership() {
        let mut app = open_test_app(true);
        app.update();

        assert!(app.world().resource::<HelpPanelState>().open);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert!(!app.world().resource::<HelpPauseGuard>().paused_by_help);
    }

    #[derive(Resource)]
    struct LateHelpIntent(Option<hw_ui::UiIntent>);

    #[derive(Resource, Default)]
    struct LogicCommitObservedUnpaused(bool);

    fn handle_late_help_intent(
        mut intent: ResMut<LateHelpIntent>,
        pending: Res<PendingWorldInputCapture>,
        content: Res<HelpPanelContent>,
        mut state: ResMut<HelpPanelState>,
        mut guard: ResMut<HelpPauseGuard>,
        mut time: ResMut<Time<Virtual>>,
        mut scroll_areas: HelpScrollAreas,
    ) {
        let Some(intent) = intent.0.take() else {
            return;
        };
        handle_help_intent(
            intent,
            &pending,
            &content,
            &mut state,
            &mut guard,
            &mut time,
            &mut scroll_areas,
        );
    }

    fn observe_settings_commit_time(
        time: Res<Time<Virtual>>,
        mut observed: ResMut<LogicCommitObservedUnpaused>,
    ) {
        observed.0 = !time.is_paused();
    }

    #[test]
    fn unread_settings_request_delays_f1_pause_until_after_same_frame_logic_commit() {
        use crate::entities::familiar::{Familiar, FamiliarOperation, FamiliarPolicy};
        use crate::systems::GameSystemSet;
        use hw_core::events::{FamiliarRosterReleasedVisualMessage, SoulTaskUnassignRequest};
        use hw_core::familiar::FamiliarSettingsPatch;

        let mut app = minimal_app();
        app.init_resource::<PendingWorldInputCapture>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<UiInputState>()
            .init_resource::<MenuState>()
            .init_resource::<HelpPanelState>()
            .init_resource::<HelpPauseGuard>()
            .init_resource::<Time<Virtual>>()
            .init_resource::<LogicCommitObservedUnpaused>()
            .insert_resource(content())
            .insert_resource(SavePath::new(
                "/definitely/missing/help-settings-race-save.ron",
            ))
            .insert_resource(InputFocus::default())
            .insert_resource(LateHelpIntent(Some(hw_ui::UiIntent::OpenHelp {
                opener: None,
            })))
            .add_message::<hw_familiar_ai::FamiliarSettingsChangeRequest>()
            .add_message::<hw_familiar_ai::FamiliarSettingsChangeOutcome>()
            .add_message::<SoulTaskUnassignRequest>()
            .add_message::<FamiliarRosterReleasedVisualMessage>()
            .configure_sets(
                Update,
                (
                    GameSystemSet::Input,
                    GameSystemSet::Logic.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                    GameSystemSet::Interface,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    reset_pending_world_input_capture_system,
                    request_capture_from_resolved_actions_system,
                    apply_accepted_help_open_system,
                )
                    .chain()
                    .in_set(GameSystemSet::Input),
            )
            .add_systems(
                Update,
                (
                    hw_familiar_ai::apply_familiar_settings_change_requests_system,
                    ApplyDeferred,
                    observe_settings_commit_time,
                )
                    .chain()
                    .in_set(GameSystemSet::Logic),
            )
            .add_systems(
                Update,
                handle_late_help_intent.in_set(GameSystemSet::Interface),
            );
        app.world_mut().spawn((
            Node {
                display: Display::None,
                ..default()
            },
            UiInputCapture,
            hw_ui::help::HelpPanel,
        ));
        app.world_mut()
            .spawn((ScrollPosition(Vec2::new(0.0, 80.0)), HelpScrollArea));
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation::default(),
                FamiliarPolicy::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::OpenHelp],
                None,
                false,
            );
        app.world_mut()
            .write_message(hw_familiar_ai::FamiliarSettingsChangeRequest {
                target: familiar,
                patch: FamiliarSettingsPatch::SetAllWorkAllowed { allowed: false },
            });

        app.update();

        assert!(
            app.world()
                .get::<FamiliarPolicy>(familiar)
                .unwrap()
                .all_work_disabled()
        );
        assert!(app.world().resource::<LogicCommitObservedUnpaused>().0);
        assert!(app.world().resource::<HelpPanelState>().open);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert!(app.world().resource::<HelpPauseGuard>().paused_by_help);

        for _ in 0..3 {
            app.update();
        }
        app.world_mut().resource_mut::<LateHelpIntent>().0 = Some(hw_ui::UiIntent::CloseHelp);
        app.update();

        assert!(!app.world().resource::<HelpPanelState>().open);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert!(
            app.world()
                .get::<FamiliarPolicy>(familiar)
                .unwrap()
                .all_work_disabled()
        );
    }
}
