use bevy::prelude::*;
use hw_core::relationships::{DeliveringTo, LoadedIn, StoredIn};
use hw_jobs::mud_mixer::StoredByMixer;

use crate::types::ResourceItem;

type ExpiredItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut ItemDespawnTimer,
        Option<&'static LoadedIn>,
        Option<&'static StoredIn>,
        Option<&'static DeliveringTo>,
        Option<&'static StoredByMixer>,
    ),
    With<ResourceItem>,
>;

/// アイテムの寿命を管理するタイマーコンポーネント
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct ItemDespawnTimer(pub Timer);

impl ItemDespawnTimer {
    pub fn new(seconds: f32) -> Self {
        Self(Timer::from_seconds(seconds, TimerMode::Once))
    }
}

/// 期限切れのアイテムを消去するシステム
/// ただし、運搬中または保管中のアイテムは対象外とする。
pub fn despawn_expired_items_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_items: ExpiredItemsQuery,
) {
    for (entity, mut timer, loaded, stored, delivering, stored_by_mixer) in q_items.iter_mut() {
        if loaded.is_some() || stored.is_some() || delivering.is_some() || stored_by_mixer.is_some()
        {
            continue;
        }

        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            info!("ITEM_LIFETIME: Despawning expired item {:?}", entity);
            commands.entity(entity).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::types::{ReservedForTask, ResourceType};

    fn expiring_item(world: &mut World) -> Entity {
        world
            .spawn((ResourceItem(ResourceType::Wood), ItemDespawnTimer::new(1.0)))
            .id()
    }

    #[test]
    fn item_lifetime_only_preserves_relationship_protected_items() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, despawn_expired_items_system);

        let (expired, legacy_reserved, loaded, stored, delivering, stored_by_mixer) = {
            let world = app.world_mut();
            let holder = world.spawn_empty().id();
            let mixer = world.spawn_empty().id();
            let expired = expiring_item(world);
            let legacy_reserved = expiring_item(world);
            let loaded = expiring_item(world);
            let stored = expiring_item(world);
            let delivering = expiring_item(world);
            let stored_by_mixer = expiring_item(world);
            world.entity_mut(legacy_reserved).insert(ReservedForTask);
            world.entity_mut(loaded).insert(LoadedIn(holder));
            world.entity_mut(stored).insert(StoredIn(holder));
            world.entity_mut(delivering).insert(DeliveringTo(holder));
            world
                .entity_mut(stored_by_mixer)
                .insert(StoredByMixer(mixer));
            (
                expired,
                legacy_reserved,
                loaded,
                stored,
                delivering,
                stored_by_mixer,
            )
        };

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(2));
        app.update();

        let world = app.world();
        assert!(world.get_entity(expired).is_err());
        assert!(world.get_entity(legacy_reserved).is_err());
        for protected in [loaded, stored, delivering, stored_by_mixer] {
            assert!(world.get_entity(protected).is_ok());
        }
    }
}
