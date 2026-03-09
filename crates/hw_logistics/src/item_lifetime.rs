use bevy::prelude::*;
use hw_core::relationships::{DeliveringTo, LoadedIn, StoredIn};
use hw_jobs::mud_mixer::StoredByMixer;

use crate::types::{ReservedForTask, ResourceItem};

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
/// ただし、タスク予約済み(ReservedForTask)や運搬中(LoadedIn, DeliveringTo等)のアイテムは対象外とする
pub fn despawn_expired_items_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_items: Query<
        (
            Entity,
            &mut ItemDespawnTimer,
            Option<&ReservedForTask>,
            Option<&LoadedIn>,
            Option<&StoredIn>,
            Option<&DeliveringTo>,
            Option<&StoredByMixer>,
        ),
        With<ResourceItem>,
    >,
) {
    for (entity, mut timer, reserved, loaded, stored, delivering, stored_by_mixer) in
        q_items.iter_mut()
    {
        if reserved.is_some()
            || loaded.is_some()
            || stored.is_some()
            || delivering.is_some()
            || stored_by_mixer.is_some()
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
