use std::time::Duration;

use bevy::ecs::message::{MessageRegistry, ShouldUpdateMessages};
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use bevy::window::{RequestRedraw, WindowEvent};
use bevy::winit::RawWinitWindowEvent;

use crate::plugins::messages::MessagesPlugin;
use crate::plugins::messages::paused_window::update_paused_window_messages;

#[derive(Message)]
struct GameplaySentinel;

#[derive(Resource, Default)]
struct PreCounts([usize; 3]);

#[derive(Resource, Default)]
struct LastCounts([usize; 3]);

#[derive(Resource, Default)]
struct GameplayCount(usize);

#[derive(SystemParam)]
struct WindowReaders<'w, 's> {
    raw: MessageReader<'w, 's, RawWinitWindowEvent>,
    window: MessageReader<'w, 's, WindowEvent>,
    redraw: MessageReader<'w, 's, RequestRedraw>,
}

impl WindowReaders<'_, '_> {
    fn read_counts(&mut self) -> [usize; 3] {
        [
            self.raw.read().count(),
            self.window.read().count(),
            self.redraw.read().count(),
        ]
    }
}

fn read_pre(mut readers: WindowReaders, mut counts: ResMut<PreCounts>) {
    for (total, observed) in counts.0.iter_mut().zip(readers.read_counts()) {
        *total += observed;
    }
}

fn read_last(mut readers: WindowReaders, mut counts: ResMut<LastCounts>) {
    for (total, observed) in counts.0.iter_mut().zip(readers.read_counts()) {
        *total += observed;
    }
}

fn read_gameplay(mut messages: MessageReader<GameplaySentinel>, mut count: ResMut<GameplayCount>) {
    count.0 += messages.read().count();
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(TimePlugin)
        .add_message::<RawWinitWindowEvent>()
        .add_message::<WindowEvent>()
        .add_message::<RequestRedraw>()
        .add_message::<GameplaySentinel>()
        .add_message::<MouseMotion>()
        .add_plugins(MessagesPlugin)
        .init_resource::<PreCounts>()
        .init_resource::<LastCounts>()
        .init_resource::<GameplayCount>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_nanos(
            16_666_667,
        )))
        .add_systems(PreUpdate, read_pre)
        .add_systems(Last, read_last)
        .add_systems(FixedUpdate, read_gameplay);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

fn write_window_messages(world: &mut World) {
    // Independent fixture writes, not a claim that every OS redraw emits all three.
    world.write_message(RawWinitWindowEvent {
        window_id: 1u64.into(),
        event: winit::event::WindowEvent::RedrawRequested,
    });
    world.write_message(WindowEvent::RequestRedraw(RequestRedraw));
    world.write_message(RequestRedraw);
}

fn lengths(world: &World) -> [usize; 3] {
    [
        world.resource::<Messages<RawWinitWindowEvent>>().len(),
        world.resource::<Messages<WindowEvent>>().len(),
        world.resource::<Messages<RequestRedraw>>().len(),
    ]
}

#[test]
fn paused_window_buffers_stay_bounded_and_frame_readers_observe_every_message_once() {
    let mut app = app();
    for frame in 1..=256 {
        write_window_messages(app.world_mut());
        app.update();

        assert!(lengths(app.world()).iter().all(|&length| length <= 2));
        assert_eq!(app.world().resource::<PreCounts>().0, [frame; 3]);
        assert_eq!(app.world().resource::<LastCounts>().0, [frame; 3]);
        assert!(matches!(
            app.world().resource::<MessageRegistry>().should_update,
            ShouldUpdateMessages::Waiting
        ));
    }
}

#[test]
fn ordinary_ready_or_always_rotation_does_not_drop_new_frame_messages() {
    for mode in [ShouldUpdateMessages::Ready, ShouldUpdateMessages::Always] {
        let mut app = app();
        for frame in 1..=64 {
            app.world_mut()
                .resource_mut::<MessageRegistry>()
                .should_update = mode;
            write_window_messages(app.world_mut());
            app.update();

            assert_eq!(app.world().resource::<PreCounts>().0, [frame; 3]);
            assert_eq!(app.world().resource::<LastCounts>().0, [frame; 3]);
            assert!(lengths(app.world()).iter().all(|&length| length <= 2));
        }
    }
}

#[test]
fn paused_frame_maintenance_preserves_gameplay_and_unselected_input_until_fixed_resume() {
    let mut app = app();
    app.world_mut().write_message(GameplaySentinel);
    app.world_mut()
        .write_message(MouseMotion { delta: Vec2::ONE });
    for _ in 0..64 {
        write_window_messages(app.world_mut());
        app.update();
    }

    assert_eq!(app.world().resource::<GameplayCount>().0, 0);
    assert_eq!(
        app.world().resource::<Messages<GameplaySentinel>>().len(),
        1
    );
    assert_eq!(app.world().resource::<Messages<MouseMotion>>().len(), 1);
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    for _ in 0..32 {
        app.update();
    }
    assert_eq!(app.world().resource::<GameplayCount>().0, 1);
    assert_eq!(app.world().resource::<PreCounts>().0, [64; 3]);
    assert_eq!(app.world().resource::<LastCounts>().0, [64; 3]);
}

#[test]
fn frame_maintenance_skips_normal_time_and_existing_global_rotation() {
    for (paused, mode) in [
        (false, ShouldUpdateMessages::Waiting),
        (true, ShouldUpdateMessages::Ready),
        (true, ShouldUpdateMessages::Always),
    ] {
        let mut app = app();
        if !paused {
            app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        }
        app.world_mut()
            .resource_mut::<MessageRegistry>()
            .should_update = mode;
        write_window_messages(app.world_mut());
        app.world_mut()
            .resource_mut::<Messages<RawWinitWindowEvent>>()
            .update();
        app.world_mut()
            .resource_mut::<Messages<WindowEvent>>()
            .update();
        app.world_mut()
            .resource_mut::<Messages<RequestRedraw>>()
            .update();
        write_window_messages(app.world_mut());

        update_paused_window_messages(app.world_mut());

        assert_eq!(lengths(app.world()), [2; 3]);
    }
}

#[test]
fn headless_world_does_not_gain_optional_window_buffers() {
    let mut world = World::new();
    update_paused_window_messages(&mut world);
    assert!(!world.contains_resource::<Time<Virtual>>());
    assert!(!world.contains_resource::<MessageRegistry>());

    world.init_resource::<Time<Virtual>>();
    world.resource_mut::<Time<Virtual>>().pause();
    world.init_resource::<MessageRegistry>();
    world.resource_mut::<MessageRegistry>().should_update = ShouldUpdateMessages::Waiting;
    world.init_resource::<Messages<RequestRedraw>>();
    world.write_message(RequestRedraw);
    update_paused_window_messages(&mut world);
    assert_eq!(world.resource::<Messages<RequestRedraw>>().len(), 1);
    update_paused_window_messages(&mut world);
    assert!(world.resource::<Messages<RequestRedraw>>().is_empty());
    assert!(!world.contains_resource::<Messages<RawWinitWindowEvent>>());
    assert!(!world.contains_resource::<Messages<WindowEvent>>());
}
