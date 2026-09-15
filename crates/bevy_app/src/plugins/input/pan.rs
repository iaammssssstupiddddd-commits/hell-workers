//! Middle-button camera gesture, independent of placement and left-button selection.
use crate::input_actions::ResolvedInputFrame;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::{GameSettings, WorldEpoch};
use hw_ui::{camera::MainCamera, components::UiInputState};

#[derive(Default)]
struct PanGesture {
    active: bool,
    suppress_until_release: bool,
    previous: Option<Vec2>,
    epoch: u64,
}

struct PanFrame {
    middle_down: bool,
    middle_started: bool,
    primary_active: bool,
    blocked: bool,
    cursor: Option<Vec2>,
    epoch: u64,
}

#[derive(Default)]
struct PanStep {
    claim: bool,
    movement: Option<(Vec2, Vec2)>,
}

impl PanGesture {
    fn step(&mut self, frame: PanFrame) -> PanStep {
        if self.epoch != frame.epoch {
            self.suppress_until_release |= self.active;
            self.active = false;
            self.previous = None;
            self.epoch = frame.epoch;
        }
        if self.suppress_until_release {
            if !frame.middle_down && !frame.primary_active {
                self.suppress_until_release = false;
            }
            return PanStep {
                claim: true,
                ..default()
            };
        }
        if self.active {
            if frame.blocked || frame.primary_active || !frame.middle_down || frame.cursor.is_none()
            {
                self.active = false;
                self.previous = None;
                self.suppress_until_release = frame.middle_down || frame.primary_active;
                return PanStep {
                    claim: true,
                    ..default()
                };
            }
            let cursor = frame.cursor.expect("checked cursor");
            let previous = self.previous.replace(cursor);
            return PanStep {
                claim: true,
                movement: previous.map(|previous| (previous, cursor)),
            };
        }
        if frame.middle_started
            && frame.middle_down
            && !frame.blocked
            && !frame.primary_active
            && let Some(cursor) = frame.cursor
        {
            self.active = true;
            self.previous = Some(cursor);
            return PanStep {
                claim: true,
                ..default()
            };
        }
        PanStep::default()
    }
}

#[derive(Resource, Default)]
pub(super) struct DedicatedPanState {
    gesture: PanGesture,
    movement: Option<(Vec2, Vec2)>,
}

#[derive(SystemParam)]
pub(super) struct PanInput<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    settings: Res<'w, GameSettings>,
    epoch: Res<'w, WorldEpoch>,
    ui: ResMut<'w, UiInputState>,
    resolved: ResMut<'w, ResolvedInputFrame>,
    state: ResMut<'w, DedicatedPanState>,
}

pub(super) fn capture_dedicated_pan(mut input: PanInput) {
    let window = input.window.single().ok();
    let frame = PanFrame {
        middle_down: input.buttons.pressed(MouseButton::Middle),
        middle_started: input.buttons.just_pressed(MouseButton::Middle),
        primary_active: [MouseButton::Left, MouseButton::Right]
            .into_iter()
            .any(|button| input.buttons.pressed(button) || input.buttons.just_released(button)),
        blocked: !input.settings.camera_mouse_pan_enabled
            || input.ui.pointer_over_ui
            || input.ui.world_input_captured
            || input.ui.text_input_blocks_keybinds()
            || input.resolved.pointer_selection_suppressed()
            || !window.is_some_and(|window| window.focused),
        cursor: window.and_then(Window::cursor_position),
        epoch: input.epoch.get(),
    };
    let step = input.state.gesture.step(frame);
    input.ui.world_pointer_claimed = step.claim;
    input.state.movement = step.movement;
    if step.claim {
        input.resolved.suppress_pointer_selection();
    }
}

pub(super) fn apply_dedicated_pan(
    mut state: ResMut<DedicatedPanState>,
    mut camera: Query<(&Camera, &GlobalTransform, &mut Transform), With<MainCamera>>,
) {
    let Some((from, to)) = state.movement.take() else {
        return;
    };
    let Ok((camera, global, mut transform)) = camera.single_mut() else {
        return;
    };
    if let (Ok(from), Ok(to)) = (
        camera.viewport_to_world_2d(global, from),
        camera.viewport_to_world_2d(global, to),
    ) {
        transform.translation += (from - to).extend(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(x: f32) -> PanFrame {
        PanFrame {
            middle_down: true,
            middle_started: false,
            primary_active: false,
            blocked: false,
            cursor: Some(Vec2::new(x, 0.0)),
            epoch: 0,
        }
    }

    #[test]
    fn dedicated_pan_claims_primary_through_release_without_mode_mutation() {
        let mut gesture = PanGesture::default();
        let step = gesture.step(PanFrame {
            middle_started: true,
            ..frame(1.0)
        });
        assert!(step.claim);
        assert!(step.movement.is_none());
        assert_eq!(
            gesture.step(frame(5.0)).movement,
            Some((Vec2::new(1.0, 0.0), Vec2::new(5.0, 0.0)))
        );
        let step = gesture.step(PanFrame {
            primary_active: true,
            ..frame(6.0)
        });
        assert!(step.claim);
        assert!(step.movement.is_none());
        assert!(
            gesture
                .step(PanFrame {
                    middle_down: false,
                    primary_active: true,
                    ..frame(7.0)
                })
                .claim
        );
        assert!(
            gesture
                .step(PanFrame {
                    middle_down: false,
                    ..frame(7.0)
                })
                .claim
        );
        assert!(
            !gesture
                .step(PanFrame {
                    middle_down: false,
                    ..frame(7.0)
                })
                .claim
        );
    }

    #[test]
    fn primary_drag_wins_and_capture_or_world_replace_cannot_resume_held_pan() {
        let mut gesture = PanGesture::default();
        assert!(
            !gesture
                .step(PanFrame {
                    middle_started: true,
                    primary_active: true,
                    ..frame(1.0)
                })
                .claim
        );
        assert!(!gesture.step(frame(2.0)).claim);
        for change_epoch in [false, true] {
            let mut gesture = PanGesture::default();
            gesture.step(PanFrame {
                middle_started: true,
                ..frame(1.0)
            });
            let epoch = u64::from(change_epoch);
            assert!(
                gesture
                    .step(PanFrame {
                        blocked: !change_epoch,
                        epoch,
                        ..frame(2.0)
                    })
                    .movement
                    .is_none()
            );
            assert!(
                gesture
                    .step(PanFrame {
                        epoch,
                        ..frame(3.0)
                    })
                    .movement
                    .is_none()
            );
        }
    }
}
