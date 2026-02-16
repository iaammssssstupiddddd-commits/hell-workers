use crate::constants::Z_ITEM_PICKUP;
use crate::systems::logistics::{ResourceItem, ResourceType};
use bevy::prelude::*;

/// 砂採取指定を消費済みに戻す。
pub fn clear_collect_sand_designation(commands: &mut Commands, source_entity: Entity) {
    commands
        .entity(source_entity)
        .remove::<crate::systems::jobs::Designation>();
    commands
        .entity(source_entity)
        .remove::<crate::systems::jobs::TaskSlots>();
    commands
        .entity(source_entity)
        .remove::<crate::systems::jobs::IssuedBy>();
}

/// 砂を指定量だけ生成し、猫車積載状態で返す。
pub fn spawn_loaded_sand_items(
    commands: &mut Commands,
    wheelbarrow: Entity,
    source_pos: Vec2,
    amount: u32,
) -> Vec<Entity> {
    let mut spawned = Vec::with_capacity(amount as usize);
    for i in 0..amount {
        let offset = Vec3::new((i as f32) * 2.0, 0.0, 0.0);
        let entity = commands
            .spawn((
                ResourceItem(ResourceType::Sand),
                Visibility::Hidden,
                Transform::from_translation(
                    Vec3::new(source_pos.x, source_pos.y, Z_ITEM_PICKUP) + offset,
                ),
                crate::relationships::LoadedIn(wheelbarrow),
                Name::new("Item (Sand, WheelbarrowCollect)"),
                crate::systems::logistics::item_lifetime::ItemDespawnTimer::new(5.0),
            ))
            .id();
        spawned.push(entity);
    }
    spawned
}

/// 骨を指定量だけ生成し、猫車積載状態で返す。
pub fn spawn_loaded_bone_items(
    commands: &mut Commands,
    wheelbarrow: Entity,
    source_pos: Vec2,
    amount: u32,
) -> Vec<Entity> {
    let mut spawned = Vec::with_capacity(amount as usize);
    for i in 0..amount {
        let offset = Vec3::new((i as f32) * 2.0, 0.0, 0.0);
        let entity = commands
            .spawn((
                ResourceItem(ResourceType::Bone),
                Visibility::Hidden,
                Transform::from_translation(
                    Vec3::new(source_pos.x, source_pos.y, Z_ITEM_PICKUP) + offset,
                ),
                crate::relationships::LoadedIn(wheelbarrow),
                Name::new("Item (Bone, WheelbarrowCollect)"),
            ))
            .id();
        spawned.push(entity);
    }
    spawned
}
