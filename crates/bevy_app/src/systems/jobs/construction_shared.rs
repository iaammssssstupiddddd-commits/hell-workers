//! Shared helpers for floor and wall construction systems.

use crate::assets::GameAssets;
use crate::systems::jobs::{Designation, Priority, TaskSlots};
use crate::systems::logistics::{ResourceItem, ResourceType};
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_ITEM_PICKUP};

/// Spawns refund item entities scattered around `center`.
///
/// Used by both floor and wall cancellation systems when returning
/// unfinished construction materials to the world.
pub fn spawn_refund_items(
    commands: &mut Commands,
    game_assets: &GameAssets,
    center: Vec2,
    resource_type: ResourceType,
    amount: u32,
) {
    if amount == 0 {
        return;
    }

    let (image, name) = match resource_type {
        ResourceType::Bone => (
            game_assets.icon_bone_small.clone(),
            "Item (Bone, Refund)",
        ),
        ResourceType::Wood => (
            game_assets.icon_wood_small.clone(),
            "Item (Wood, Refund)",
        ),
        ResourceType::StasisMud => (
            game_assets.icon_stasis_mud_small.clone(),
            "Item (StasisMud, Refund)",
        ),
        _ => return,
    };

    let columns = 8usize;
    for i in 0..amount as usize {
        let col = (i % columns) as f32;
        let row = (i / columns) as f32;
        let offset_x = (col - (columns as f32 - 1.0) * 0.5) * (TILE_SIZE * 0.18);
        let offset_y = row * (TILE_SIZE * 0.18);
        commands.spawn((
            ResourceItem(resource_type),
            Sprite {
                image: image.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                center.x + offset_x,
                center.y + offset_y,
                Z_ITEM_PICKUP,
            )),
            Name::new(name),
        ));
    }
}

/// Removes [`Designation`], [`TaskSlots`], and [`Priority`] from a list of tile entities.
///
/// Called at the end of phase transitions to clear task components before the
/// designation system re-adds them for the new phase.
pub fn remove_tile_task_components(commands: &mut Commands, tile_entities: &[Entity]) {
    for &entity in tile_entities {
        commands
            .entity(entity)
            .remove::<(Designation, TaskSlots, Priority)>();
    }
}
