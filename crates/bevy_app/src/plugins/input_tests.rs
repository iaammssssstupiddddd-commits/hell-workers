use super::*;
use crate::input_actions::InputModifiers;
use crate::test_support::minimal_app;
use bevy::camera::{ComputedCameraValues, RenderTarget, RenderTargetInfo};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::picking::backend::HitData;
use bevy::picking::pointer::{Location, PointerId};
use bevy::picking::prelude::{Drag, DragEnd, DragStart, Pointer, PointerButton};
use bevy::window::{WindowRef, WindowResolution};

fn pan_camera_guard_app() -> App {
    let mut app = minimal_app();
    app.init_resource::<UiInputState>()
        .init_resource::<TaskContext>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ResolvedInputFrame>()
        .init_resource::<SelectedEntity>()
        .init_resource::<TaskAreaPointerClaim>()
        .insert_resource(State::new(PlayMode::Normal))
        .init_resource::<NextState<PlayMode>>()
        .add_systems(
            PreUpdate,
            pan_camera_world_input_guard_system
                .in_set(InputPreUpdateSet::CameraGuard)
                .before(PickingSystems::Hover),
        );
    app
}

#[derive(Resource, Default)]
struct PendingTestDrag(Option<(Entity, Location, Vec2)>);

fn trigger_pending_test_drag(mut commands: Commands, mut pending: ResMut<PendingTestDrag>) {
    let Some((window, location, delta)) = pending.0.take() else {
        return;
    };
    commands.trigger(Pointer::new(
        PointerId::Mouse,
        location,
        Drag {
            button: PointerButton::Primary,
            distance: delta,
            delta,
        },
        window,
    ));
}

fn queue_drag(app: &mut App, window: Entity, location: &Location, delta: Vec2) {
    app.world_mut().resource_mut::<PendingTestDrag>().0 = Some((window, location.clone(), delta));
}

fn pointer_hit(camera: Entity) -> HitData {
    HitData {
        camera,
        depth: 0.0,
        position: None,
        normal: None,
        extra: None,
    }
}

fn trigger_drag_start(app: &mut App, window: Entity, camera: Entity, location: &Location) {
    app.world_mut().commands().trigger(Pointer::new(
        PointerId::Mouse,
        location.clone(),
        DragStart {
            button: PointerButton::Primary,
            hit: pointer_hit(camera),
        },
        window,
    ));
    app.world_mut().flush();
}

fn trigger_drag_end(app: &mut App, window: Entity, location: &Location) {
    app.world_mut().commands().trigger(Pointer::new(
        PointerId::Mouse,
        location.clone(),
        DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
        },
        window,
    ));
    app.world_mut().flush();
}

fn pan_camera_drag_app() -> (App, Entity, Entity, Location) {
    let mut app = pan_camera_guard_app();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<PendingTestDrag>()
        .add_plugins(PanCameraPlugin)
        .add_systems(
            PreUpdate,
            trigger_pending_test_drag.in_set(PickingSystems::Hover),
        )
        .add_systems(
            Update,
            handle_mouse_input.run_if(in_state(PlayMode::Normal)),
        );

    let mut window = Window {
        resolution: WindowResolution::new(100, 100),
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(50.0)));
    let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();
    let camera_entity = app
        .world_mut()
        .spawn((
            Camera {
                computed: ComputedCameraValues {
                    clip_from_view: Mat4::IDENTITY,
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::splat(100),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                ..default()
            },
            Transform::default(),
            GlobalTransform::IDENTITY,
            PanCamera::default(),
            MainCamera,
        ))
        .id();
    let location = Location {
        target: RenderTarget::Window(WindowRef::Primary)
            .normalize(Some(window_entity))
            .unwrap(),
        position: Vec2::splat(50.0),
    };

    (app, window_entity, camera_entity, location)
}

#[test]
fn resolved_render_debug_actions_reach_each_existing_owner() {
    let mut app = minimal_app();
    app.init_resource::<ResolvedInputFrame>()
        .init_resource::<crate::Render3dVisible>()
        .init_resource::<QualitySettings>()
        .init_resource::<crate::RenderPerfToggles>()
        .init_resource::<crate::DebugVisible>()
        .init_resource::<GizmoConfigStore>()
        .init_resource::<hw_core::GameSettings>()
        .add_systems(
            Update,
            (
                render3d_toggle_system,
                rtt_quality_cycle_system,
                rtt_directional_light_toggle_system,
                rtt_terrain_toggle_system,
                rtt_scene_objects_toggle_system,
                debug_toggle_system,
            ),
        );
    let expected_quality = app.world().resource::<QualitySettings>().rtt.next();
    let initial_directional = app
        .world()
        .resource::<crate::RenderPerfToggles>()
        .directional_light_enabled;
    let initial_terrain = app
        .world()
        .resource::<crate::RenderPerfToggles>()
        .terrain_enabled;
    let initial_scene_objects = app
        .world()
        .resource::<crate::RenderPerfToggles>()
        .scene_objects_enabled;
    app.world_mut()
        .resource_mut::<ResolvedInputFrame>()
        .replace(
            InputModifiers::default(),
            vec![
                InputAction::ToggleRender3d,
                InputAction::CycleRttQuality,
                InputAction::ToggleRttDirectionalLight,
                InputAction::ToggleRttTerrain,
                InputAction::ToggleRttSceneObjects,
                InputAction::ToggleDebug,
            ],
            None,
            false,
        );

    app.update();

    assert!(!app.world().resource::<crate::Render3dVisible>().0);
    assert_eq!(
        app.world().resource::<QualitySettings>().rtt,
        expected_quality
    );
    let perf = app.world().resource::<crate::RenderPerfToggles>();
    assert_eq!(perf.directional_light_enabled, !initial_directional);
    assert_eq!(perf.terrain_enabled, !initial_terrain);
    assert_eq!(perf.scene_objects_enabled, !initial_scene_objects);
    assert!(app.world().resource::<crate::DebugVisible>().0);
    assert!(
        app.world()
            .resource::<hw_core::GameSettings>()
            .debug_gizmos_enabled
    );
}

#[test]
fn pan_camera_capture_guard_restores_enabled_without_changing_mouse_setting() {
    let mut app = pan_camera_guard_app();
    let mut controller = PanCamera::default();
    controller.mouse_pan_settings.enabled = false;
    let camera = app.world_mut().spawn((controller, MainCamera)).id();
    app.world_mut()
        .resource_mut::<UiInputState>()
        .world_input_captured = true;

    app.update();

    let controller = app.world().entity(camera).get::<PanCamera>().unwrap();
    assert!(!controller.enabled);
    assert!(!controller.mouse_pan_settings.enabled);

    app.world_mut()
        .resource_mut::<UiInputState>()
        .world_input_captured = false;
    app.update();

    let controller = app.world().entity(camera).get::<PanCamera>().unwrap();
    assert!(controller.enabled);
    assert!(!controller.mouse_pan_settings.enabled);
}

#[test]
fn pan_camera_guard_blocks_task_area_drag_from_press_through_release() {
    let mut app = pan_camera_guard_app();
    let camera = app
        .world_mut()
        .spawn((PanCamera::default(), MainCamera))
        .id();
    app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::AreaSelection(None);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);

    app.update();

    assert!(
        !app.world()
            .entity(camera)
            .get::<PanCamera>()
            .unwrap()
            .enabled
    );

    app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::None;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.update();

    assert!(
        !app.world()
            .entity(camera)
            .get::<PanCamera>()
            .unwrap()
            .enabled
    );

    {
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.clear();
        buttons.release(MouseButton::Left);
    }
    app.update();

    assert!(
        !app.world()
            .entity(camera)
            .get::<PanCamera>()
            .unwrap()
            .enabled
    );

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.update();

    assert!(
        app.world()
            .entity(camera)
            .get::<PanCamera>()
            .unwrap()
            .enabled
    );
}

#[test]
fn pan_camera_guard_preclaims_resolved_familiar_area_action() {
    let mut app = pan_camera_guard_app();
    let camera = app
        .world_mut()
        .spawn((PanCamera::default(), MainCamera))
        .id();
    app.world_mut()
        .resource_mut::<ResolvedInputFrame>()
        .replace(
            InputModifiers::default(),
            vec![InputAction::FamiliarChop],
            None,
            true,
        );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);

    app.update();

    assert!(
        !app.world()
            .entity(camera)
            .get::<PanCamera>()
            .unwrap()
            .enabled
    );
}

#[test]
fn direct_task_area_border_drag_keeps_actual_pan_camera_transform_stable_until_release() {
    let (mut app, window, camera, location) = pan_camera_drag_app();
    let familiar = app
        .world_mut()
        .spawn((
            Familiar::default(),
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(32.0)),
        ))
        .id();
    app.world_mut().resource_mut::<SelectedEntity>().0 = Some(familiar);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);

    let original_transform = *app.world().entity(camera).get::<Transform>().unwrap();
    trigger_drag_start(&mut app, window, camera, &location);
    queue_drag(&mut app, window, &location, Vec2::new(10.0, 0.0));
    app.update();

    assert_eq!(
        *app.world().entity(camera).get::<Transform>().unwrap(),
        original_transform
    );
    assert_eq!(
        app.world().resource::<TaskContext>().0,
        TaskMode::AreaSelection(None)
    );

    app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::None;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    queue_drag(&mut app, window, &location, Vec2::new(10.0, 0.0));
    app.update();

    assert_eq!(
        *app.world().entity(camera).get::<Transform>().unwrap(),
        original_transform
    );

    {
        let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        buttons.clear();
        buttons.release(MouseButton::Left);
    }
    app.update();
    trigger_drag_end(&mut app, window, &location);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.update();

    trigger_drag_start(&mut app, window, camera, &location);
    queue_drag(&mut app, window, &location, Vec2::new(10.0, 0.0));
    app.update();

    assert_ne!(
        *app.world().entity(camera).get::<Transform>().unwrap(),
        original_transform
    );
}

#[test]
fn all_task_area_drag_modes_reserve_the_primary_pointer() {
    let point = Vec2::splat(1.0);
    let zone_type = hw_core::game_state::TaskModeZoneType::Stockpile;
    for mode in [
        TaskMode::DesignateChop(None),
        TaskMode::DesignateMine(Some(point)),
        TaskMode::DesignateHaul(None),
        TaskMode::CancelDesignation(Some(point)),
        TaskMode::AreaSelection(None),
        TaskMode::AssignTask(Some(point)),
        TaskMode::ZonePlacement(zone_type, None),
        TaskMode::ZoneRemoval(zone_type, Some(point)),
        TaskMode::FloorPlace(None),
        TaskMode::WallPlace(Some(point)),
        TaskMode::DreamPlanting(None),
        TaskMode::StockpilePolicyEdit(Some(point)),
    ] {
        assert!(task_mode_uses_area_drag(mode), "mode: {mode:?}");
    }

    assert!(!task_mode_uses_area_drag(TaskMode::None));
    assert!(!task_mode_uses_area_drag(TaskMode::SelectBuildTarget));
    assert!(!task_mode_uses_area_drag(TaskMode::SoulSpaPlace(None)));
}

#[test]
fn direct_area_probe_uses_the_state_seen_by_update() {
    let current = State::new(PlayMode::TaskDesignation);
    let mut next = NextState::default();
    next.set(PlayMode::Normal);

    assert!(normal_pointer_ingress_will_run(&current, &next));

    next.set(PlayMode::FloorPlace);
    assert!(!normal_pointer_ingress_will_run(&current, &next));
}
