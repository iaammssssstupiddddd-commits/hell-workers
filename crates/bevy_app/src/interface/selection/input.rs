use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::Familiar;
use crate::input_actions::ResolvedInputFrame;
use crate::interface::ui::UiInputState;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::GameSettings;
use hw_core::game_state::PlayMode;
use hw_core::selection::{
    FamiliarMoveFeedback, SelectionCandidate, SelectionTargetClass, WorldPointerTarget,
};
use hw_ui::camera::MainCamera;
use hw_ui::selection::{OpenWorldContextMenu, SelectionIntent};
use std::time::{Duration, Instant};

use super::hit_test::SelectionResolver;
use super::hit_test::hovered_task_area_border_entity;
use super::state::{HoveredEntity, SelectedEntity};

const CLICK_SLOP_LOGICAL_PX: f32 = 5.0;

pub(crate) fn pointer_hits_task_area_border(
    world_pos: Vec2,
    current_selected: Option<Entity>,
    q_task_areas: &Query<(Entity, &TaskArea), With<Familiar>>,
) -> bool {
    hovered_task_area_border_entity(world_pos, current_selected, q_task_areas).is_some()
}

#[derive(Resource, Debug, Default)]
pub struct WorldSelectionGesture {
    pending: Option<PendingSelection>,
    suppressed_until_release: bool,
    last_click_at: Option<Instant>,
    last_click_pos: Vec2,
    last_candidates: Vec<Entity>,
    last_candidate_index: usize,
}

#[derive(Debug)]
struct PendingSelection {
    start_screen_pos: Vec2,
    last_screen_pos: Vec2,
    candidates: Vec<SelectionCandidate>,
    dragged: bool,
    pan_started: bool,
}

#[derive(SystemParam)]
pub struct SelectionInput<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    pub ui_input_state: Res<'w, UiInputState>,
    pub resolved_frame: Res<'w, ResolvedInputFrame>,
}

#[derive(SystemParam)]
pub(crate) struct SelectionMutation<'w, 's> {
    gesture: ResMut<'w, WorldSelectionGesture>,
    selected_entity: ResMut<'w, SelectedEntity>,
    next_play_mode: ResMut<'w, NextState<PlayMode>>,
    task_context: ResMut<'w, TaskContext>,
    q_dest: Query<'w, 's, &'static mut Destination>,
    q_familiars: Query<'w, 's, (), With<Familiar>>,
    context_requests: MessageWriter<'w, OpenWorldContextMenu>,
    settings: Res<'w, GameSettings>,
    q_camera_transform: Query<'w, 's, &'static mut Transform, With<MainCamera>>,
    move_feedback: ResMut<'w, FamiliarMoveFeedback>,
}

pub(crate) fn handle_mouse_input(
    input: SelectionInput,
    resolver: SelectionResolver,
    state: SelectionMutation,
) {
    let SelectionMutation {
        mut gesture,
        mut selected_entity,
        mut next_play_mode,
        mut task_context,
        mut q_dest,
        q_familiars,
        mut context_requests,
        settings,
        mut q_camera_transform,
        mut move_feedback,
    } = state;
    let blocked = input.ui_input_state.world_input_blocked()
        || input.resolved_frame.pointer_selection_suppressed();

    if blocked {
        if input.buttons.pressed(MouseButton::Left) {
            gesture.pending = None;
            gesture.suppressed_until_release = true;
        }
        if input.buttons.just_released(MouseButton::Left) {
            gesture.suppressed_until_release = false;
        }
        return;
    }

    let Ok(window) = input.q_window.single() else {
        gesture.pending = None;
        return;
    };
    let Some(screen_pos) = window.cursor_position() else {
        if input.buttons.pressed(MouseButton::Left) {
            gesture.pending = None;
            gesture.suppressed_until_release = true;
        }
        return;
    };
    let Ok((camera, camera_transform)) = input.q_camera.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, screen_pos) else {
        return;
    };

    if input.buttons.just_pressed(MouseButton::Left) && !gesture.suppressed_until_release {
        let candidates = resolver.resolve(
            screen_pos,
            world_pos,
            camera,
            camera_transform,
            selected_entity.0,
        );
        if candidates
            .first()
            .is_some_and(|candidate| candidate.class == SelectionTargetClass::TaskArea)
        {
            let familiar = candidates[0].entity;
            apply_selection_intent(
                SelectionIntent::StartAreaSelection { familiar },
                &mut selected_entity,
                &mut next_play_mode,
                &mut task_context,
                &mut q_dest,
                &mut move_feedback,
            );
            gesture.pending = None;
        } else {
            gesture.pending = Some(PendingSelection {
                start_screen_pos: screen_pos,
                last_screen_pos: screen_pos,
                candidates,
                dragged: false,
                pan_started: false,
            });
        }
    }

    if let Some(pending) = gesture.pending.as_mut() {
        let total_delta = screen_pos - pending.start_screen_pos;
        if exceeds_click_slop(pending.start_screen_pos, screen_pos) {
            pending.dragged = true;
            if settings.camera_mouse_pan_enabled {
                let pan_from = if pending.pan_started {
                    pending.last_screen_pos
                } else {
                    pending.start_screen_pos
                        + total_delta.normalize_or_zero() * CLICK_SLOP_LOGICAL_PX
                };
                if let (Ok(from_world), Ok(to_world), Ok(mut transform)) = (
                    camera.viewport_to_world_2d(camera_transform, pan_from),
                    camera.viewport_to_world_2d(camera_transform, screen_pos),
                    q_camera_transform.single_mut(),
                ) {
                    transform.translation += (from_world - to_world).extend(0.0);
                }
                pending.pan_started = true;
            }
        }
        pending.last_screen_pos = screen_pos;
    }

    if input.buttons.just_released(MouseButton::Left) {
        if gesture.suppressed_until_release {
            gesture.suppressed_until_release = false;
            gesture.pending = None;
        } else if let Some(pending) = gesture.pending.take()
            && !pending.dragged
        {
            let live_candidates = resolver.resolve(
                screen_pos,
                world_pos,
                camera,
                camera_transform,
                selected_entity.0,
            );
            let latched = cycled_latched_candidate(&mut gesture, pending);
            let still_live = latched.filter(|candidate| {
                live_candidates
                    .iter()
                    .any(|live| live.entity == candidate.entity && live.class == candidate.class)
            });
            let intent = selection_intent(still_live);
            apply_selection_intent(
                intent,
                &mut selected_entity,
                &mut next_play_mode,
                &mut task_context,
                &mut q_dest,
                &mut move_feedback,
            );
        }
    }

    if input.buttons.just_pressed(MouseButton::Right) && !input.buttons.pressed(MouseButton::Left) {
        let candidates = resolver.resolve(
            screen_pos,
            world_pos,
            camera,
            camera_transform,
            selected_entity.0,
        );
        let context_target = candidates.iter().find(|candidate| {
            !matches!(
                candidate.class,
                SelectionTargetClass::Floor | SelectionTargetClass::TaskArea
            )
        });
        if let Some(target) = context_target {
            context_requests.write(OpenWorldContextMenu {
                target: target.entity,
                screen_pos,
            });
        } else if let Some(familiar) = selected_entity.0
            && q_familiars.get(familiar).is_ok()
        {
            apply_selection_intent(
                SelectionIntent::MoveFamiliar {
                    familiar,
                    destination: world_pos,
                },
                &mut selected_entity,
                &mut next_play_mode,
                &mut task_context,
                &mut q_dest,
                &mut move_feedback,
            );
        }
    }
}

fn exceeds_click_slop(start: Vec2, current: Vec2) -> bool {
    start.distance(current) > CLICK_SLOP_LOGICAL_PX
}

fn cycled_latched_candidate(
    gesture: &mut WorldSelectionGesture,
    pending: PendingSelection,
) -> Option<SelectionCandidate> {
    let now = Instant::now();
    let entity_order: Vec<_> = pending
        .candidates
        .iter()
        .map(|candidate| candidate.entity)
        .collect();
    let repeats_stack = gesture.last_click_at.is_some_and(|last| {
        now.duration_since(last) <= Duration::from_millis(500)
            && pending.start_screen_pos.distance(gesture.last_click_pos) <= 4.0
            && entity_order == gesture.last_candidates
    });
    let index = if repeats_stack && !pending.candidates.is_empty() {
        (gesture.last_candidate_index + 1) % pending.candidates.len()
    } else {
        0
    };
    gesture.last_click_at = Some(now);
    gesture.last_click_pos = pending.start_screen_pos;
    gesture.last_candidates = entity_order;
    gesture.last_candidate_index = index;
    pending.candidates.get(index).copied()
}

fn selection_intent(candidate: Option<SelectionCandidate>) -> SelectionIntent {
    match candidate {
        Some(candidate) if candidate.class == SelectionTargetClass::TaskArea => {
            SelectionIntent::StartAreaSelection {
                familiar: candidate.entity,
            }
        }
        Some(candidate) => SelectionIntent::Select(candidate.entity),
        None => SelectionIntent::ClearSelection,
    }
}

fn apply_selection_intent(
    intent: SelectionIntent,
    selected_entity: &mut SelectedEntity,
    next_play_mode: &mut NextState<PlayMode>,
    task_context: &mut TaskContext,
    q_dest: &mut Query<&mut Destination>,
    move_feedback: &mut FamiliarMoveFeedback,
) {
    match intent {
        SelectionIntent::Select(entity) => selected_entity.0 = Some(entity),
        SelectionIntent::ClearSelection => selected_entity.0 = None,
        SelectionIntent::StartAreaSelection { familiar } => {
            selected_entity.0 = Some(familiar);
            task_context.0 = TaskMode::AreaSelection(None);
            next_play_mode.set(PlayMode::TaskDesignation);
        }
        SelectionIntent::MoveFamiliar {
            familiar,
            destination,
        } => {
            if let Ok(mut dest) = q_dest.get_mut(familiar) {
                dest.0 = destination;
                move_feedback.accepted(destination);
            }
        }
        SelectionIntent::None => {}
    }
}

pub(crate) fn update_hover_entity(
    input: SelectionInput,
    resolver: SelectionResolver,
    selected_entity: Res<SelectedEntity>,
    mut hovered_entity: ResMut<HoveredEntity>,
    mut pointer_target: ResMut<WorldPointerTarget>,
) {
    if input.ui_input_state.world_input_blocked() {
        hovered_entity.0 = None;
        *pointer_target = WorldPointerTarget::default();
        return;
    }
    let Ok(window) = input.q_window.single() else {
        return;
    };
    let Some(screen_pos) = window.cursor_position() else {
        hovered_entity.0 = None;
        *pointer_target = WorldPointerTarget::default();
        return;
    };
    let Ok((camera, camera_transform)) = input.q_camera.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, screen_pos) else {
        return;
    };
    let candidates = resolver.resolve(
        screen_pos,
        world_pos,
        camera,
        camera_transform,
        selected_entity.0,
    );
    let primary = candidates.first().copied();
    hovered_entity.0 = primary.map(|candidate| candidate.entity);
    pointer_target.primary = primary;
    pointer_target.candidates = candidates;
    pointer_target.screen_pos = Some(screen_pos);
    pointer_target.world_pos = Some(world_pos);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::selection::SelectionHitKind;

    fn candidate(entity: Entity) -> SelectionCandidate {
        SelectionCandidate {
            entity,
            class: SelectionTargetClass::Object,
            hit: SelectionHitKind::Direct,
            distance_px: 0.0,
            depth: 0.0,
        }
    }

    #[test]
    fn five_pixels_is_a_click_and_more_is_a_drag() {
        assert!(!exceeds_click_slop(Vec2::ZERO, Vec2::new(3.0, 4.0)));
        assert!(exceeds_click_slop(Vec2::ZERO, Vec2::new(3.1, 4.0)));
    }

    #[test]
    fn repeated_click_cycles_the_stable_candidate_stack() {
        let first = candidate(Entity::from_bits(1));
        let second = candidate(Entity::from_bits(2));
        let mut gesture = WorldSelectionGesture::default();
        let pending = || PendingSelection {
            start_screen_pos: Vec2::splat(20.0),
            last_screen_pos: Vec2::splat(20.0),
            candidates: vec![first, second],
            dragged: false,
            pan_started: false,
        };

        assert_eq!(
            cycled_latched_candidate(&mut gesture, pending()).map(|value| value.entity),
            Some(first.entity)
        );
        assert_eq!(
            cycled_latched_candidate(&mut gesture, pending()).map(|value| value.entity),
            Some(second.entity)
        );
    }
}
