//! Pointer owner for the player-facing single-building deconstruction gesture.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::WorldEpoch;
use hw_core::game_state::PlayMode;
use hw_jobs::DeconstructionDesignationRequest;
use hw_ui::camera::{MainCamera, world_cursor_pos};

use crate::app_contexts::TaskContext;
use crate::input_actions::ResolvedInputFrame;
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskMode;
use crate::world::map::WorldMap;

#[derive(SystemParam)]
pub struct DeconstructionDesignationInputParams<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<'w, UiInputState>,
    resolved_frame: Res<'w, ResolvedInputFrame>,
    world_map: Res<'w, WorldMap>,
    world_epoch: Res<'w, WorldEpoch>,
    task_context: ResMut<'w, TaskContext>,
    next_play_mode: ResMut<'w, NextState<PlayMode>>,
    requests: MessageWriter<'w, DeconstructionDesignationRequest>,
    next_request_id: Local<'s, u64>,
}

pub fn deconstruction_designation_input_system(mut params: DeconstructionDesignationInputParams) {
    if !matches!(params.task_context.0, TaskMode::DesignateDeconstruct(_)) {
        return;
    }

    let world_input_blocked = params.ui_input_state.world_input_blocked()
        || params.resolved_frame.pointer_selection_suppressed();
    if world_input_blocked {
        if params.buttons.just_released(MouseButton::Left)
            && matches!(
                params.task_context.0,
                TaskMode::DesignateDeconstruct(Some(_))
            )
        {
            params.task_context.0 = TaskMode::DesignateDeconstruct(None);
        }
        return;
    }

    if params.buttons.just_pressed(MouseButton::Right) {
        params.task_context.0 = TaskMode::None;
        params.next_play_mode.set(PlayMode::Normal);
        return;
    }

    let cursor_world = world_cursor_pos(&params.q_window, &params.q_camera);
    let hit = cursor_world.and_then(|world_pos| {
        let grid = WorldMap::world_to_grid(world_pos);
        params
            .world_map
            .building_entity(grid)
            .or_else(|| params.world_map.floor_entity(grid))
    });
    let (next_mode, request) = advance_deconstruction_pointer(
        params.task_context.0,
        params.buttons.just_pressed(MouseButton::Left),
        params.buttons.just_released(MouseButton::Left),
        cursor_world,
        hit,
        *params.world_epoch,
        &mut params.next_request_id,
    );
    params.task_context.0 = next_mode;
    if let Some(request) = request {
        params.requests.write(request);
    }
}

fn advance_deconstruction_pointer(
    mut mode: TaskMode,
    left_just_pressed: bool,
    left_just_released: bool,
    cursor_world: Option<Vec2>,
    hit: Option<Entity>,
    world_epoch: WorldEpoch,
    next_request_id: &mut u64,
) -> (TaskMode, Option<DeconstructionDesignationRequest>) {
    if left_just_pressed
        && matches!(mode, TaskMode::DesignateDeconstruct(None))
        && let Some(cursor_world) = cursor_world
    {
        mode = TaskMode::DesignateDeconstruct(Some(WorldMap::snap_to_grid_center(cursor_world)));
    }

    if !left_just_released || !matches!(mode, TaskMode::DesignateDeconstruct(Some(_))) {
        return (mode, None);
    }

    *next_request_id = next_request_id.wrapping_add(1).max(1);
    (
        TaskMode::DesignateDeconstruct(None),
        Some(DeconstructionDesignationRequest {
            request_id: *next_request_id,
            world_epoch: world_epoch.get(),
            hit,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::InputModifiers;

    fn input_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<UiInputState>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<WorldMap>()
            .init_resource::<WorldEpoch>()
            .init_resource::<TaskContext>()
            .init_resource::<NextState<PlayMode>>()
            .add_message::<DeconstructionDesignationRequest>()
            .add_systems(Update, deconstruction_designation_input_system);
        app
    }

    #[test]
    fn one_press_release_emits_one_request_and_keeps_the_mode_active() {
        let target = Entity::from_raw_u32(7).expect("fixture entity");
        let mut request_id = 0;

        let (pressed, request) = advance_deconstruction_pointer(
            TaskMode::DesignateDeconstruct(None),
            true,
            false,
            Some(Vec2::new(17.0, 33.0)),
            Some(target),
            WorldEpoch::default(),
            &mut request_id,
        );
        assert!(matches!(pressed, TaskMode::DesignateDeconstruct(Some(_))));
        assert_eq!(request, None);

        let (released, request) = advance_deconstruction_pointer(
            pressed,
            false,
            true,
            Some(Vec2::new(17.0, 33.0)),
            Some(target),
            WorldEpoch::default(),
            &mut request_id,
        );
        assert_eq!(released, TaskMode::DesignateDeconstruct(None));
        assert_eq!(
            request,
            Some(DeconstructionDesignationRequest {
                request_id: 1,
                world_epoch: 0,
                hit: Some(target),
            })
        );

        let (idle, duplicate) = advance_deconstruction_pointer(
            released,
            false,
            true,
            None,
            None,
            WorldEpoch::default(),
            &mut request_id,
        );
        assert_eq!(idle, TaskMode::DesignateDeconstruct(None));
        assert_eq!(duplicate, None);
        assert_eq!(request_id, 1);
    }

    #[test]
    fn releasing_outside_the_window_still_produces_a_typed_no_target_receipt() {
        let mut request_id = 9;
        let (mode, request) = advance_deconstruction_pointer(
            TaskMode::DesignateDeconstruct(Some(Vec2::ZERO)),
            false,
            true,
            None,
            None,
            WorldEpoch::default(),
            &mut request_id,
        );

        assert_eq!(mode, TaskMode::DesignateDeconstruct(None));
        assert_eq!(request.unwrap().hit, None);
        assert_eq!(request_id, 10);
    }

    #[test]
    fn blocked_release_rolls_back_capture_without_emitting_a_request() {
        let mut app = input_app();
        app.world_mut().resource_mut::<TaskContext>().0 =
            TaskMode::DesignateDeconstruct(Some(Vec2::ONE));
        app.world_mut()
            .resource_mut::<UiInputState>()
            .pointer_over_ui = true;
        {
            let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            buttons.press(MouseButton::Left);
            buttons.release(MouseButton::Left);
        }

        app.update();

        assert_eq!(
            app.world().resource::<TaskContext>().0,
            TaskMode::DesignateDeconstruct(None)
        );
        assert!(
            app.world()
                .resource::<Messages<DeconstructionDesignationRequest>>()
                .iter_current_update_messages()
                .next()
                .is_none()
        );
    }

    #[test]
    fn right_click_exits_the_mode_without_needing_a_world_cursor() {
        let mut app = input_app();
        app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::DesignateDeconstruct(None);
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(InputModifiers::default(), Vec::new(), None, false);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);

        app.update();

        assert_eq!(app.world().resource::<TaskContext>().0, TaskMode::None);
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::Normal) | NextState::PendingIfNeq(PlayMode::Normal)
        ));
    }
}
