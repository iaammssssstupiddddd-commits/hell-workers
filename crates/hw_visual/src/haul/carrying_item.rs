//! 運搬中アイテムのビジュアル表示システム

use bevy::prelude::*;

use super::components::{CarryingItemVisual, HasCarryingIndicator};
use super::{CARRIED_ITEM_ICON_SIZE, CARRIED_ITEM_Y_OFFSET};
use crate::handles::{HaulItemHandles, MaterialIconHandles, WorkIconHandles};
use hw_core::logistics::ResourceType;
use hw_core::soul::DamnedSoul;
use hw_logistics::types::{Inventory, ResourceItem};

pub fn spawn_carrying_item_system(
    mut commands: Commands,
    mat_handles: Res<MaterialIconHandles>,
    haul_handles: Res<HaulItemHandles>,
    q_workers: Query<
        (Entity, &Transform, &Inventory),
        (With<DamnedSoul>, Without<HasCarryingIndicator>),
    >,
    q_items: Query<&ResourceItem>,
) {
    for (worker_entity, transform, inventory) in q_workers.iter() {
        let Some(item_entity) = inventory.0 else {
            continue;
        };
        let Ok(item) = q_items.get(item_entity) else {
            continue;
        };

        let icon_handle = match item.0 {
            ResourceType::Wood => mat_handles.wood_small.clone(),
            ResourceType::Rock => mat_handles.rock_small.clone(),
            ResourceType::Water => mat_handles.water_small.clone(),
            ResourceType::BucketEmpty => haul_handles.bucket_empty.clone(),
            ResourceType::BucketWater => haul_handles.bucket_water.clone(),
            ResourceType::Sand => haul_handles.sand_pile.clone(),
            ResourceType::Bone => mat_handles.bone_small.clone(),
            ResourceType::StasisMud => haul_handles.stasis_mud.clone(),
            ResourceType::Wheelbarrow => continue,
        };

        info!(
            "VISUAL: Spawning carrying icon for worker {:?} ({:?})",
            worker_entity, item.0
        );

        let icon_pos = transform.translation + Vec3::new(0.0, CARRIED_ITEM_Y_OFFSET, 0.5);

        commands.spawn((
            CarryingItemVisual {
                worker: worker_entity,
            },
            Sprite {
                image: icon_handle,
                custom_size: Some(Vec2::splat(CARRIED_ITEM_ICON_SIZE)),
                ..default()
            },
            Transform::from_translation(icon_pos),
            Name::new("CarryingItemVisual"),
        ));

        commands
            .entity(worker_entity)
            .try_insert(HasCarryingIndicator);
    }
}

pub fn update_carrying_item_system(
    mut commands: Commands,
    mat_handles: Res<MaterialIconHandles>,
    haul_handles: Res<HaulItemHandles>,
    work_handles: Res<WorkIconHandles>,
    q_workers: Query<(Entity, &Transform, &Inventory), With<DamnedSoul>>,
    q_items: Query<&ResourceItem>,
    mut q_icons: Query<
        (Entity, &CarryingItemVisual, &mut Transform, &mut Sprite),
        Without<DamnedSoul>,
    >,
) {
    for (icon_entity, icon, mut icon_transform, mut icon_sprite) in q_icons.iter_mut() {
        let mut should_despawn = true;

        if let Ok((_, worker_transform, inventory)) = q_workers.get(icon.worker) {
            if let Some(item_entity) = inventory.0 {
                should_despawn = false;

                icon_transform.translation =
                    worker_transform.translation + Vec3::new(0.0, CARRIED_ITEM_Y_OFFSET, 0.5);

                if let Ok(item) = q_items.get(item_entity) {
                    let new_icon_handle = match item.0 {
                        ResourceType::Wood => mat_handles.wood_small.clone(),
                        ResourceType::Rock => mat_handles.rock_small.clone(),
                        ResourceType::Water => mat_handles.water_small.clone(),
                        ResourceType::BucketEmpty => haul_handles.bucket_empty.clone(),
                        ResourceType::BucketWater => haul_handles.bucket_water.clone(),
                        ResourceType::Sand => haul_handles.sand_pile.clone(),
                        ResourceType::Bone => mat_handles.bone_small.clone(),
                        ResourceType::StasisMud => haul_handles.stasis_mud.clone(),
                        ResourceType::Wheelbarrow => work_handles.wheelbarrow_small.clone(),
                    };
                    if icon_sprite.image != new_icon_handle {
                        icon_sprite.image = new_icon_handle;
                    }
                }
            }
        }

        if should_despawn {
            info!(
                "VISUAL: Despawning carrying icon for worker {:?}",
                icon.worker
            );
            commands.entity(icon_entity).try_despawn();
            if let Ok(mut worker_commands) = commands.get_entity(icon.worker) {
                worker_commands.try_remove::<HasCarryingIndicator>();
            }
        }
    }
}
