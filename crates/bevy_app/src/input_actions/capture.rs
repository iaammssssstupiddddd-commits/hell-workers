use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use hw_core::game_state::TimeSpeed;
use hw_ui::components::{
    LoadConfirmDialog, MenuAction, MenuButton, MenuState, OperationDialog, PauseMenu,
    SettingsPanel, UiInputCapture, UiInputState,
};

use super::{InputAction, InputOverlay, ResolvedInputFrame};
use crate::entities::familiar::Familiar;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::list::reset_entity_list_drag_state;
use crate::systems::save::SavePath;

use super::ActiveModeCleanupParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorldInputCaptureRequest {
    overlay: InputOverlay,
    root: Entity,
    opener: Option<Entity>,
}

/// Frame-local bridge between an accepted open request and overlay visibility.
#[derive(Resource, Debug, Default)]
pub struct PendingWorldInputCapture {
    request: Option<WorldInputCaptureRequest>,
}

impl PendingWorldInputCapture {
    pub(crate) fn overlay(&self) -> Option<InputOverlay> {
        self.request.map(|request| request.overlay)
    }

    fn request(&mut self, request: WorldInputCaptureRequest) {
        let replace = self
            .request
            .is_none_or(|current| request.overlay.priority() > current.overlay.priority());
        if replace {
            self.request = Some(request);
        }
    }

    fn foreground_opener(&self, foreground_root: Option<Entity>) -> Option<Entity> {
        self.request
            .filter(|request| Some(request.root) == foreground_root)
            .and_then(|request| request.opener)
    }
}

type CaptureRootQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Node,
        Has<LoadConfirmDialog>,
        Has<SettingsPanel>,
        Has<PauseMenu>,
        Has<OperationDialog>,
    ),
    With<UiInputCapture>,
>;

type CaptureOpeningButtonQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Interaction, &'static MenuButton),
    (Changed<Interaction>, With<Button>),
>;

fn capture_root_overlay(
    is_load: bool,
    is_settings: bool,
    is_pause: bool,
    is_operation: bool,
) -> Option<InputOverlay> {
    if is_load {
        Some(InputOverlay::LoadConfirm)
    } else if is_settings {
        Some(InputOverlay::Settings)
    } else if is_pause {
        Some(InputOverlay::Pause)
    } else if is_operation {
        Some(InputOverlay::OperationDialog)
    } else {
        None
    }
}

fn root_for_overlay(roots: &CaptureRootQuery<'_, '_>, overlay: InputOverlay) -> Option<Entity> {
    roots.iter().find_map(
        |(entity, _, is_load, is_settings, is_pause, is_operation)| {
            (capture_root_overlay(is_load, is_settings, is_pause, is_operation) == Some(overlay))
                .then_some(entity)
        },
    )
}

fn visible_capture(
    roots: &CaptureRootQuery<'_, '_>,
    simulation_paused: bool,
) -> Option<(InputOverlay, Entity)> {
    roots
        .iter()
        .filter_map(
            |(entity, node, is_load, is_settings, is_pause, is_operation)| {
                let overlay = capture_root_overlay(is_load, is_settings, is_pause, is_operation)?;
                let visible = node.display != Display::None
                    || (overlay == InputOverlay::Pause && simulation_paused);
                visible.then_some((overlay, entity))
            },
        )
        .max_by_key(|(overlay, _)| overlay.priority())
}

fn begin_world_input_capture(
    overlay: InputOverlay,
    opener: Option<Entity>,
    roots: &CaptureRootQuery<'_, '_>,
    pending: &mut PendingWorldInputCapture,
    input_focus: &mut InputFocus,
) -> bool {
    let Some(root) = root_for_overlay(roots, overlay) else {
        return false;
    };
    pending.request(WorldInputCaptureRequest {
        overlay,
        root,
        opener,
    });
    input_focus.clear();
    true
}

pub(crate) fn reset_pending_world_input_capture_system(
    mut pending: ResMut<PendingWorldInputCapture>,
) {
    pending.request = None;
}

#[derive(SystemParam)]
pub(crate) struct CaptureRequestParams<'w, 's> {
    pending: ResMut<'w, PendingWorldInputCapture>,
    input_focus: ResMut<'w, InputFocus>,
    time: Res<'w, Time<Virtual>>,
    menu_state: Res<'w, MenuState>,
    save_path: Res<'w, SavePath>,
    selected: Res<'w, SelectedEntity>,
    familiars: Query<'w, 's, (), With<Familiar>>,
    roots: CaptureRootQuery<'w, 's>,
}

fn capture_overlay_for_menu_action(
    action: MenuAction,
    params: &CaptureRequestParams<'_, '_>,
) -> Option<InputOverlay> {
    match action {
        MenuAction::RequestLoadGame if params.save_path.as_path().exists() => {
            Some(InputOverlay::LoadConfirm)
        }
        MenuAction::ToggleSettings if *params.menu_state != MenuState::Settings => {
            Some(InputOverlay::Settings)
        }
        MenuAction::TogglePause if !params.time.is_paused() => Some(InputOverlay::Pause),
        MenuAction::SetTimeSpeed(TimeSpeed::Paused) if !params.time.is_paused() => {
            Some(InputOverlay::Pause)
        }
        MenuAction::OpenOperationDialog
            if params
                .selected
                .0
                .is_some_and(|entity| params.familiars.get(entity).is_ok()) =>
        {
            Some(InputOverlay::OperationDialog)
        }
        _ => None,
    }
}

/// Accepts capture-opening UI buttons before keyboard resolution for this frame.
pub(crate) fn request_capture_from_menu_buttons_system(
    buttons: CaptureOpeningButtonQuery,
    mut params: CaptureRequestParams,
) {
    for (entity, interaction, menu_button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(overlay) = capture_overlay_for_menu_action(menu_button.0, &params) else {
            continue;
        };
        begin_world_input_capture(
            overlay,
            Some(entity),
            &params.roots,
            &mut params.pending,
            &mut params.input_focus,
        );
    }
}

/// Converts capture-opening keyboard actions into the same pending request path.
pub(crate) fn request_capture_from_resolved_actions_system(
    mut resolved_frame: ResMut<ResolvedInputFrame>,
    mut params: CaptureRequestParams,
) {
    let overlay = if resolved_frame.contains(InputAction::RequestLoadGame)
        && params.save_path.as_path().exists()
    {
        Some(InputOverlay::LoadConfirm)
    } else if (resolved_frame.contains(InputAction::TogglePause)
        || resolved_frame.contains(InputAction::TimePaused))
        && !params.time.is_paused()
    {
        Some(InputOverlay::Pause)
    } else {
        None
    };

    if let Some(overlay) = overlay
        && begin_world_input_capture(
            overlay,
            None,
            &params.roots,
            &mut params.pending,
            &mut params.input_focus,
        )
    {
        resolved_frame.suppress_pointer_selection();
    }
}

/// Publishes the effective capture state from pending requests and visible roots.
pub(crate) fn sync_world_input_capture_system(
    pending: Res<PendingWorldInputCapture>,
    roots: CaptureRootQuery<'_, '_>,
    time: Res<Time<Virtual>>,
    mut ui_input_state: ResMut<UiInputState>,
    mut resolved_frame: ResMut<ResolvedInputFrame>,
) {
    let pending_capture = pending
        .request
        .map(|request| (request.overlay, request.root));
    let visible_capture = visible_capture(&roots, time.is_paused());
    let foreground = match (pending_capture, visible_capture) {
        (Some(pending), Some(visible)) => {
            if pending.0.priority() >= visible.0.priority() {
                Some(pending)
            } else {
                Some(visible)
            }
        }
        (Some(pending), None) => Some(pending),
        (None, Some(visible)) => Some(visible),
        (None, None) => None,
    };

    let was_captured = ui_input_state.world_input_captured;
    ui_input_state.world_input_captured = foreground.is_some();
    ui_input_state.world_input_capture_started = !was_captured && foreground.is_some();
    ui_input_state.foreground_capture_root = foreground.map(|(_, root)| root);

    if ui_input_state.world_input_captured {
        resolved_frame.suppress_pointer_selection();
    }
}

/// Restores uncommitted pointer-owner state exactly once when capture starts.
pub(crate) fn rollback_in_progress_gesture_system(
    ui_input_state: Res<UiInputState>,
    mut cleanup: ActiveModeCleanupParams,
    drag_state: Option<ResMut<hw_ui::list::DragState>>,
    resize_state: Option<ResMut<hw_ui::list::EntityListResizeState>>,
) {
    if !ui_input_state.world_input_capture_started {
        return;
    }

    cleanup.rollback_in_progress_gesture();
    if let Some(mut drag_state) = drag_state {
        reset_entity_list_drag_state(&mut cleanup.commands, &mut drag_state);
    }
    if let Some(mut resize_state) = resize_state {
        resize_state.reset_active();
    }
}

/// Returns whether a pressed UI entity belongs to the current foreground overlay.
pub(crate) fn foreground_ui_action_allowed(
    entity: Entity,
    ui_input_state: &UiInputState,
    pending: &PendingWorldInputCapture,
    parents: &Query<&ChildOf>,
) -> bool {
    if !ui_input_state.world_input_captured {
        return true;
    }
    if pending.foreground_opener(ui_input_state.foreground_capture_root) == Some(entity) {
        return true;
    }

    let Some(root) = ui_input_state.foreground_capture_root else {
        return false;
    };
    let mut current = entity;
    for _ in 0..64 {
        if current == root {
            return true;
        }
        let Ok(parent) = parents.get(current) else {
            return false;
        };
        current = parent.parent();
    }
    false
}

#[derive(SystemParam)]
pub struct ForegroundUiGate<'w, 's> {
    ui_input_state: Res<'w, UiInputState>,
    pending: Res<'w, PendingWorldInputCapture>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl ForegroundUiGate<'_, '_> {
    pub(crate) fn allows(&self, entity: Entity) -> bool {
        foreground_ui_action_allowed(entity, &self.ui_input_state, &self.pending, &self.parents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::minimal_app;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn capture_test_app() -> App {
        let mut app = minimal_app();
        app.init_resource::<PendingWorldInputCapture>()
            .init_resource::<UiInputState>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<Time<Virtual>>()
            .init_resource::<MenuState>()
            .init_resource::<SelectedEntity>()
            .insert_resource(SavePath::new(PathBuf::from(
                "/definitely/missing/hell-workers-test-save.ron",
            )))
            .insert_resource(InputFocus::from_entity(Entity::PLACEHOLDER));
        app
    }

    fn spawn_capture_root<T: Component>(app: &mut App, marker: T, display: Display) -> Entity {
        app.world_mut()
            .spawn((
                Node {
                    display,
                    ..default()
                },
                UiInputCapture,
                marker,
            ))
            .id()
    }

    fn assert_accepted_button_capture<T: Component>(
        mut app: App,
        marker: T,
        action: MenuAction,
        expected_overlay: InputOverlay,
    ) {
        let root = spawn_capture_root(&mut app, marker, Display::None);
        let opener = app
            .world_mut()
            .spawn((Interaction::Pressed, Button, MenuButton(action)))
            .id();
        app.add_systems(
            Update,
            (
                reset_pending_world_input_capture_system,
                request_capture_from_menu_buttons_system,
            )
                .chain(),
        );

        app.update();

        let request = app
            .world()
            .resource::<PendingWorldInputCapture>()
            .request
            .expect("capture request should be accepted");
        assert_eq!(request.overlay, expected_overlay);
        assert_eq!(request.root, root);
        assert_eq!(request.opener, Some(opener));
        assert!(app.world().resource::<InputFocus>().get().is_none());
    }

    #[test]
    fn accepted_button_request_clears_focus_while_rejected_close_preserves_it() {
        let mut accepted = capture_test_app();
        let root = spawn_capture_root(&mut accepted, SettingsPanel, Display::None);
        let opener = accepted
            .world_mut()
            .spawn((
                Interaction::Pressed,
                Button,
                MenuButton(MenuAction::ToggleSettings),
            ))
            .id();
        accepted.add_systems(
            Update,
            (
                reset_pending_world_input_capture_system,
                request_capture_from_menu_buttons_system,
            )
                .chain(),
        );

        accepted.update();

        let request = accepted
            .world()
            .resource::<PendingWorldInputCapture>()
            .request
            .expect("settings request should be accepted");
        assert_eq!(request.overlay, InputOverlay::Settings);
        assert_eq!(request.root, root);
        assert_eq!(request.opener, Some(opener));
        assert!(accepted.world().resource::<InputFocus>().get().is_none());

        let mut rejected = capture_test_app();
        *rejected.world_mut().resource_mut::<MenuState>() = MenuState::Settings;
        spawn_capture_root(&mut rejected, SettingsPanel, Display::Flex);
        rejected.world_mut().spawn((
            Interaction::Pressed,
            Button,
            MenuButton(MenuAction::ToggleSettings),
        ));
        rejected.add_systems(
            Update,
            (
                reset_pending_world_input_capture_system,
                request_capture_from_menu_buttons_system,
            )
                .chain(),
        );

        rejected.update();

        assert!(
            rejected
                .world()
                .resource::<PendingWorldInputCapture>()
                .request
                .is_none()
        );
        assert_eq!(
            rejected.world().resource::<InputFocus>().get(),
            Some(Entity::PLACEHOLDER)
        );
    }

    #[test]
    fn accepted_button_requests_cover_every_capture_overlay() {
        assert_accepted_button_capture(
            capture_test_app(),
            SettingsPanel,
            MenuAction::ToggleSettings,
            InputOverlay::Settings,
        );
        assert_accepted_button_capture(
            capture_test_app(),
            PauseMenu,
            MenuAction::TogglePause,
            InputOverlay::Pause,
        );

        let mut operation = capture_test_app();
        let familiar = operation.world_mut().spawn(Familiar::default()).id();
        operation.world_mut().resource_mut::<SelectedEntity>().0 = Some(familiar);
        assert_accepted_button_capture(
            operation,
            OperationDialog,
            MenuAction::OpenOperationDialog,
            InputOverlay::OperationDialog,
        );

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let save_file = std::env::temp_dir().join(format!(
            "hell-workers-capture-{}-{unique}.ron",
            std::process::id()
        ));
        std::fs::write(&save_file, b"capture test").unwrap();
        let mut load = capture_test_app();
        load.insert_resource(SavePath::new(save_file.clone()));
        assert_accepted_button_capture(
            load,
            LoadConfirmDialog,
            MenuAction::RequestLoadGame,
            InputOverlay::LoadConfirm,
        );
        std::fs::remove_file(save_file).unwrap();
    }

    #[test]
    fn rejected_load_operation_and_resume_requests_preserve_focus() {
        let mut app = capture_test_app();
        spawn_capture_root(&mut app, LoadConfirmDialog, Display::None);
        spawn_capture_root(&mut app, OperationDialog, Display::None);
        spawn_capture_root(&mut app, PauseMenu, Display::Flex);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        for action in [
            MenuAction::RequestLoadGame,
            MenuAction::OpenOperationDialog,
            MenuAction::TogglePause,
        ] {
            app.world_mut()
                .spawn((Interaction::Pressed, Button, MenuButton(action)));
        }
        app.add_systems(
            Update,
            (
                reset_pending_world_input_capture_system,
                request_capture_from_menu_buttons_system,
            )
                .chain(),
        );

        app.update();

        assert!(
            app.world()
                .resource::<PendingWorldInputCapture>()
                .request
                .is_none()
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(Entity::PLACEHOLDER)
        );
    }

    #[test]
    fn accepted_keyboard_pause_request_clears_focus_and_suppresses_pointer_ingress() {
        let mut app = capture_test_app();
        let root = spawn_capture_root(&mut app, PauseMenu, Display::None);
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                super::super::InputModifiers::default(),
                vec![InputAction::TogglePause],
                None,
                false,
            );
        app.add_systems(Update, request_capture_from_resolved_actions_system);

        app.update();

        let request = app
            .world()
            .resource::<PendingWorldInputCapture>()
            .request
            .unwrap();
        assert_eq!(request.overlay, InputOverlay::Pause);
        assert_eq!(request.root, root);
        assert!(app.world().resource::<InputFocus>().get().is_none());
        assert!(
            app.world()
                .resource::<ResolvedInputFrame>()
                .pointer_selection_suppressed()
        );
    }

    #[test]
    fn pending_to_visible_handoff_keeps_capture_without_restarting_latch() {
        let mut app = capture_test_app();
        let root = spawn_capture_root(&mut app, SettingsPanel, Display::None);
        app.world_mut()
            .resource_mut::<PendingWorldInputCapture>()
            .request(WorldInputCaptureRequest {
                overlay: InputOverlay::Settings,
                root,
                opener: None,
            });
        app.add_systems(Update, sync_world_input_capture_system);

        app.update();
        let state = app.world().resource::<UiInputState>();
        assert!(state.world_input_captured);
        assert!(state.world_input_capture_started);
        assert_eq!(state.foreground_capture_root, Some(root));

        app.world_mut()
            .resource_mut::<PendingWorldInputCapture>()
            .request = None;
        app.world_mut()
            .entity_mut(root)
            .get_mut::<Node>()
            .unwrap()
            .display = Display::Flex;
        app.update();
        let state = app.world().resource::<UiInputState>();
        assert!(state.world_input_captured);
        assert!(!state.world_input_capture_started);

        app.world_mut()
            .entity_mut(root)
            .get_mut::<Node>()
            .unwrap()
            .display = Display::None;
        app.update();
        let state = app.world().resource::<UiInputState>();
        assert!(!state.world_input_captured);
        assert!(!state.world_input_capture_started);
        assert_eq!(state.foreground_capture_root, None);
    }

    #[derive(Resource, Default)]
    struct CaptureStartCount(usize);

    fn count_capture_start(state: Res<UiInputState>, mut count: ResMut<CaptureStartCount>) {
        if state.world_input_capture_started {
            count.0 += 1;
        }
    }

    #[test]
    fn pending_to_visible_handoff_raises_capture_start_exactly_once() {
        let mut app = capture_test_app();
        let root = spawn_capture_root(&mut app, SettingsPanel, Display::None);
        app.init_resource::<CaptureStartCount>().add_systems(
            Update,
            (sync_world_input_capture_system, count_capture_start).chain(),
        );
        app.world_mut()
            .resource_mut::<PendingWorldInputCapture>()
            .request(WorldInputCaptureRequest {
                overlay: InputOverlay::Settings,
                root,
                opener: None,
            });

        app.update();
        app.world_mut()
            .resource_mut::<PendingWorldInputCapture>()
            .request = None;
        app.world_mut()
            .entity_mut(root)
            .get_mut::<Node>()
            .unwrap()
            .display = Display::Flex;
        app.update();

        assert_eq!(app.world().resource::<CaptureStartCount>().0, 1);
    }

    #[test]
    fn capture_priority_selects_the_highest_root_across_pending_and_visible() {
        let mut app = capture_test_app();
        let operation = spawn_capture_root(&mut app, OperationDialog, Display::Flex);
        let settings = spawn_capture_root(&mut app, SettingsPanel, Display::None);
        let load = spawn_capture_root(&mut app, LoadConfirmDialog, Display::None);
        app.add_systems(Update, sync_world_input_capture_system);
        app.world_mut()
            .resource_mut::<PendingWorldInputCapture>()
            .request(WorldInputCaptureRequest {
                overlay: InputOverlay::Settings,
                root: settings,
                opener: None,
            });

        app.update();
        assert_eq!(
            app.world()
                .resource::<UiInputState>()
                .foreground_capture_root,
            Some(settings)
        );

        app.world_mut()
            .entity_mut(load)
            .get_mut::<Node>()
            .unwrap()
            .display = Display::Flex;
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiInputState>()
                .foreground_capture_root,
            Some(load)
        );
        assert_ne!(operation, settings);
    }

    #[derive(Resource)]
    struct GateTargets {
        descendant: Entity,
        opener: Entity,
        background: Entity,
    }

    #[derive(Resource, Default)]
    struct GateResults(Vec<bool>);

    fn evaluate_gate(
        targets: Res<GateTargets>,
        state: Res<UiInputState>,
        pending: Res<PendingWorldInputCapture>,
        parents: Query<&ChildOf>,
        mut results: ResMut<GateResults>,
    ) {
        results.0 = [targets.descendant, targets.opener, targets.background]
            .into_iter()
            .map(|entity| foreground_ui_action_allowed(entity, &state, &pending, &parents))
            .collect();
    }

    #[test]
    fn foreground_gate_accepts_root_descendants_and_winning_opener_only() {
        let mut app = capture_test_app();
        let root = spawn_capture_root(&mut app, SettingsPanel, Display::None);
        let panel = app.world_mut().spawn(ChildOf(root)).id();
        let descendant = app.world_mut().spawn(ChildOf(panel)).id();
        let opener = app.world_mut().spawn_empty().id();
        let background = app.world_mut().spawn_empty().id();
        {
            let mut state = app.world_mut().resource_mut::<UiInputState>();
            state.world_input_captured = true;
            state.foreground_capture_root = Some(root);
        }
        app.world_mut()
            .resource_mut::<PendingWorldInputCapture>()
            .request(WorldInputCaptureRequest {
                overlay: InputOverlay::Settings,
                root,
                opener: Some(opener),
            });
        app.insert_resource(GateTargets {
            descendant,
            opener,
            background,
        })
        .init_resource::<GateResults>()
        .add_systems(Update, evaluate_gate);

        app.update();

        assert_eq!(app.world().resource::<GateResults>().0, [true, true, false]);
    }
}
