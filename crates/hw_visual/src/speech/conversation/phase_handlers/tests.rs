use super::*;
use crate::test_log::WarningProbe;
use bevy::ecs::{error::FallbackErrorHandler, system::RunSystemOnce};
use bevy::time::{
    DelayedCommandQueue, TimePlugin, TimeUpdateStrategy, check_delayed_command_queues,
};
use std::time::Duration;

fn test_world() -> World {
    WarningProbe::start();
    let mut world = World::new();
    world.init_resource::<Time>();
    world.insert_resource(FallbackErrorHandler(bevy::ecs::error::panic));
    world
}

fn finish(world: &mut World, soul: Entity, duration: Option<f32>) {
    end_conversation(&mut world.commands(), soul, duration);
    world.flush();
}

#[test]
fn conversation_cooldown_expires_only_after_its_deadline() {
    let mut world = test_world();
    let soul = world.spawn(DamnedSoul::default()).id();
    finish(&mut world, soul, Some(1.0));
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(500));
    world.run_system_once(check_delayed_command_queues).unwrap();
    assert!(world.get::<ConversationCooldown>(soul).is_some());
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(500));
    world.run_system_once(check_delayed_command_queues).unwrap();
    assert!(world.get::<ConversationCooldown>(soul).is_none());
    WarningProbe::assert_clean();
}

#[test]
fn conversation_cooldown_does_not_touch_reused_entity_after_world_replace() {
    let mut world = test_world();
    let old = world.spawn(DamnedSoul::default()).id();
    finish(&mut world, old, Some(1.0));
    crate::reset_for_world_replace(&mut world);
    let next = world.try_despawn_no_free(old).unwrap();
    let replacement = world
        .spawn_at(next, (DamnedSoul::default(), ConversationCooldown))
        .unwrap()
        .id();
    assert_eq!(old.index(), replacement.index());
    assert_ne!(old, replacement);
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    world.run_system_once(check_delayed_command_queues).unwrap();
    assert!(world.get::<ConversationCooldown>(replacement).is_some());
    assert_eq!(
        world.query::<&DelayedCommandQueue>().iter(&world).count(),
        0
    );
    WarningProbe::assert_clean();
}

#[test]
fn conversation_cooldown_pause_preserves_deadline_and_resume_rejects_old_owner() {
    WarningProbe::start();
    let mut app = App::new();
    app.add_plugins(TimePlugin)
        .insert_resource(FallbackErrorHandler(bevy::ecs::error::panic))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            250,
        )));
    app.update();
    let old = app.world_mut().spawn(DamnedSoul::default()).id();
    finish(app.world_mut(), old, Some(1.0));
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let elapsed = app.world().resource::<Time>().elapsed();
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(app.world().resource::<Time>().elapsed(), elapsed);
    assert!(app.world().get::<ConversationCooldown>(old).is_some());
    crate::reset_for_world_replace(app.world_mut());
    app.world_mut().despawn(old);
    let replacement = app
        .world_mut()
        .spawn((DamnedSoul::default(), ConversationCooldown))
        .id();
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world()
            .get::<ConversationCooldown>(replacement)
            .is_some()
    );
    assert_eq!(
        app.world_mut()
            .query::<&DelayedCommandQueue>()
            .iter(app.world())
            .count(),
        0
    );
    WarningProbe::assert_clean();
}

#[test]
fn interrupted_conversation_cooldown_has_no_delayed_queue() {
    let mut world = test_world();
    let soul = world.spawn(DamnedSoul::default()).id();
    finish(&mut world, soul, None);
    assert!(world.get::<ConversationCooldown>(soul).is_none());
    assert_eq!(
        world.query::<&DelayedCommandQueue>().iter(&world).count(),
        0
    );
    WarningProbe::assert_clean();
}
