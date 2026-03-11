use bevy::prelude::*;

const DRAG_HOLD_SECONDS: f32 = 0.2;

#[derive(Resource)]
pub struct DragState {
    pub pending_soul: Option<Entity>,
    pub hold_timer: Timer,
    pub active_soul: Option<Entity>,
    pub drop_target: Option<Entity>,
    pub ghost_entity: Option<Entity>,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            pending_soul: None,
            hold_timer: Timer::from_seconds(DRAG_HOLD_SECONDS, TimerMode::Once),
            active_soul: None,
            drop_target: None,
            ghost_entity: None,
        }
    }
}

impl DragState {
    pub fn is_dragging(&self) -> bool {
        self.active_soul.is_some()
    }

    pub fn drop_target(&self) -> Option<Entity> {
        self.drop_target
    }

    pub fn reset_hold_timer(&mut self) {
        self.hold_timer = Timer::from_seconds(DRAG_HOLD_SECONDS, TimerMode::Once);
        self.hold_timer.reset();
    }
}
