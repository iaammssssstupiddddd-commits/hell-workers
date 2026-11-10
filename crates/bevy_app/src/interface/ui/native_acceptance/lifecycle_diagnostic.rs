//! Temporary bounded, read-only native diagnosis; remove after ownership diagnosis.
use bevy::prelude::*;

fn bounded(text: &str) -> (String, bool) {
    (text.chars().take(64).collect(), text.chars().count() > 64)
}

fn slot(count: &mut usize) -> bool {
    if *count >= 4096 {
        if *count == 4096 {
            info!("NATIVE_DESPAWN limit=4096 reached; later removals are unobserved");
            *count += 1;
        }
        return false;
    }
    *count += 1;
    true
}

pub(super) fn trace_despawn(
    on: On<Despawn, (Transform, Node, Name, Text2d, Text)>,
    entities: Query<EntityRef>,
    mut count: Local<usize>,
) {
    if !slot(&mut count) {
        return;
    }
    let Ok(entity) = entities.get(on.entity) else {
        return;
    };
    info!(
        "NATIVE_DESPAWN index={} entity={:?} parent={:?} name={:?} text2d={:?} text={:?} speech={} background={} sprite={} node={} components={:?}",
        *count,
        on.entity,
        entity.get::<ChildOf>().map(ChildOf::parent),
        entity.get::<Name>().map(|n| bounded(n.as_str())),
        entity.get::<Text2d>().map(|t| bounded(&t.0)),
        entity.get::<Text>().map(|t| bounded(&t.0)),
        entity.contains::<hw_visual::speech::components::SpeechBubble>(),
        entity.contains::<hw_visual::speech::components::SpeechBubbleBackground>(),
        entity.contains::<Sprite>(),
        entity.contains::<Node>(),
        entity.archetype().components()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Resource, Default)]
    struct Seen(Vec<Entity>);
    #[test]
    fn diagnostic_has_finite_rows_and_utf8_fields() {
        let mut count = 0;
        assert_eq!((0..5000).filter(|_| slot(&mut count)).count(), 4096);
        assert_eq!(count, 4097);
        assert_eq!(bounded("short"), ("short".into(), false));
        let text = "猫".repeat(100);
        assert_eq!(bounded(&text), ("猫".repeat(64), true));
    }
    #[test]
    fn typed_despawn_observer_reads_the_live_entity_once() {
        let mut app = App::new();
        app.init_resource::<Seen>();
        app.add_observer(
            |on: On<Despawn, (Name, Transform)>,
             q: Query<EntityRef, Without<bevy::ecs::resource::IsResource>>,
             mut seen: ResMut<Seen>| {
                assert!(q.get(on.entity).is_ok());
                seen.0.push(on.entity);
            },
        );
        let e = app
            .world_mut()
            .spawn((Name::new("probe"), Transform::default()))
            .id();
        app.world_mut().despawn(e);
        assert_eq!(app.world().resource::<Seen>().0, [e]);
    }
}
