//! Frame-owned window buffers must age even when fixed simulation is paused.

use bevy::ecs::message::{MessageRegistry, ShouldUpdateMessages};
use bevy::prelude::*;
use bevy::window::{RequestRedraw, WindowEvent};
use bevy::winit::RawWinitWindowEvent;

pub(super) fn update_paused_window_messages(world: &mut World) {
    let paused = world
        .get_resource::<Time<Virtual>>()
        .is_some_and(|time| time.is_paused());
    let waiting = world
        .get_resource::<MessageRegistry>()
        .is_some_and(|registry| matches!(registry.should_update, ShouldUpdateMessages::Waiting));
    if !paused || !waiting {
        return;
    }

    // TimePlugin waits for FixedPostUpdate before rotating the global registry.
    // These three channels have frame/runner consumers, not fixed consumers.
    // Ready/Always must use the ordinary update alone, preserving new messages.
    update_buffer::<RawWinitWindowEvent>(world);
    update_buffer::<WindowEvent>(world);
    update_buffer::<RequestRedraw>(world);
}

fn update_buffer<T: Message>(world: &mut World) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<T>>() {
        messages.update();
    }
}
