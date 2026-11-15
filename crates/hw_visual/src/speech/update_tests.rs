use super::*;
use crate::handles::SpeechHandles;
use crate::speech::conversation::events::ConversationToneTriggered;
use crate::speech::observers::speech_on_soul_recruited_system;
use crate::test_log::WarningProbe;
use bevy::ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};
use hw_core::{events::SoulRecruitedVisualMessage, familiar::Familiar};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct SpeechTick;

#[test]
fn recruitment_and_expiry_share_one_old_bubble_without_stale_commands() {
    for reverse in [false, true] {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SoulRecruitedVisualMessage>()
            .add_message::<ConversationToneTriggered>()
            .insert_resource(SpeechHandles {
                bubble_9slice: Handle::default(),
                glow_circle: Handle::default(),
                font_familiar: Handle::default(),
                font_soul_name: Handle::default(),
                font_soul_emoji: Handle::default(),
            });
        let mut schedule = Schedule::new(SpeechTick);
        schedule.set_executor(SingleThreadedExecutor::new());
        // Reproduce the formerly permitted order: both systems observe the old
        // entity before either deferred queue is applied.
        if reverse {
            schedule.add_systems(
                (update_speech_bubbles, speech_on_soul_recruited_system).chain_ignore_deferred(),
            );
        } else {
            schedule.add_systems(
                (speech_on_soul_recruited_system, update_speech_bubbles).chain_ignore_deferred(),
            );
        }
        app.add_schedule(schedule);
        let familiar = app
            .world_mut()
            .spawn((Familiar::default(), GlobalTransform::default()))
            .id();
        let soul = app.world_mut().spawn_empty().id();
        let background = app.world_mut().spawn_empty().id();
        let old = app
            .world_mut()
            .spawn((
                SpeechBubble {
                    elapsed: 1.,
                    duration: 0.1,
                    speaker: familiar,
                    offset: Vec2::ZERO,
                    emotion: BubbleEmotion::Neutral,
                    background: Some(background),
                },
                FamiliarBubble,
                Transform::default(),
                ChildOf(familiar),
            ))
            .id();
        app.world_mut().entity_mut(background).insert(ChildOf(old));
        app.world_mut().write_message(SoulRecruitedVisualMessage {
            entity: soul,
            familiar_entity: familiar,
        });
        WarningProbe::start();
        app.world_mut().run_schedule(SpeechTick);
        WarningProbe::assert_clean();
        assert!(app.world().get_entity(old).is_err());
        assert!(app.world().get_entity(background).is_err());
        let remaining = app
            .world_mut()
            .query_filtered::<(Entity, &SpeechBubble), With<FamiliarBubble>>()
            .iter(app.world())
            .map(|(e, b)| (e, b.speaker))
            .collect::<Vec<_>>();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].1, familiar);
    }
}

fn expired_fixture() -> (App, Entity, Entity, Entity) {
    let mut app = App::new();
    app.init_resource::<Time>();
    let speaker = app.world_mut().spawn_empty().id();
    let background = app.world_mut().spawn_empty().id();
    let bubble = app
        .world_mut()
        .spawn((
            SpeechBubble {
                elapsed: 1.,
                duration: 0.1,
                speaker,
                offset: Vec2::ZERO,
                emotion: BubbleEmotion::Neutral,
                background: Some(background),
            },
            Transform::default(),
            ChildOf(speaker),
        ))
        .id();
    app.world_mut()
        .entity_mut(background)
        .insert(ChildOf(bubble));
    (app, speaker, bubble, background)
}

#[test]
fn expiry_rechecks_deadline_at_apply_and_does_not_remove_a_refreshed_bubble() {
    fn refresh(mut bubbles: Query<&mut SpeechBubble>) {
        for mut bubble in &mut bubbles {
            bubble.duration = 100.;
        }
    }
    let (mut app, _, bubble, background) = expired_fixture();
    let mut schedule = Schedule::new(SpeechTick);
    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.add_systems((update_speech_bubbles, refresh).chain_ignore_deferred());
    app.add_schedule(schedule);
    WarningProbe::start();
    app.world_mut().run_schedule(SpeechTick);
    WarningProbe::assert_clean();
    assert!(app.world().get::<SpeechBubble>(bubble).is_some());
    assert_eq!(
        app.world().get::<ChildOf>(background).unwrap().parent(),
        bubble
    );
}

#[test]
fn speaker_cleanup_and_expiry_remove_the_owned_tree_without_stale_child_commands() {
    fn remove(mut commands: Commands, bubbles: Query<&SpeechBubble>) {
        for bubble in &bubbles {
            commands.entity(bubble.speaker).despawn();
        }
    }
    let (mut app, speaker, bubble, background) = expired_fixture();
    let mut schedule = Schedule::new(SpeechTick);
    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.add_systems((remove, update_speech_bubbles).chain_ignore_deferred());
    app.add_schedule(schedule);
    WarningProbe::start();
    app.world_mut().run_schedule(SpeechTick);
    WarningProbe::assert_clean();
    for entity in [speaker, bubble, background] {
        assert!(app.world().get_entity(entity).is_err());
    }
}
