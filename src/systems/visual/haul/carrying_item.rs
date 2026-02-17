//! 運搬中アイテムのビジュアル表示システム

use bevy::prelude::*;

use super::components::{CarryingItemVisual, HasCarryingIndicator};
use super::{CARRIED_ITEM_ICON_SIZE, CARRIED_ITEM_Y_OFFSET};
use crate::assets::GameAssets;
use crate::entities::damned_soul::DamnedSoul;
use crate::systems::logistics::{Inventory, ResourceItem, ResourceType};

/// 運搬中のワーカーにアイテムアイコンを付与する
pub fn spawn_carrying_item_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_workers: Query<
        (Entity, &Transform, &Inventory),
        (With<DamnedSoul>, Without<HasCarryingIndicator>),
    >,
    q_items: Query<&ResourceItem>,
) {
    for (worker_entity, transform, inventory) in q_workers.iter() {
        // Inventoryにあるアイテムのタイプを取得
        let Some(item_entity) = inventory.0 else {
            continue;
        };
        let Ok(item) = q_items.get(item_entity) else {
            continue;
        };

        // リソースタイプに応じたアイコンを選択
        let icon_handle = match item.0 {
            ResourceType::Wood => game_assets.icon_wood_small.clone(),
            ResourceType::Rock => game_assets.icon_rock_small.clone(),
            ResourceType::Water => game_assets.icon_water_small.clone(),
            ResourceType::BucketEmpty => game_assets.bucket_empty.clone(),
            ResourceType::BucketWater => game_assets.bucket_water.clone(),
            ResourceType::Sand => game_assets.sand_pile.clone(),
            ResourceType::Bone => game_assets.icon_bone_small.clone(),
            ResourceType::StasisMud => game_assets.stasis_mud.clone(),
            ResourceType::Wheelbarrow => continue, // 手押し車自体は頭上アイコン不要
        };

        info!(
            "VISUAL: Spawning carrying icon for worker {:?} ({:?})",
            worker_entity, item.0
        );

        let icon_pos = transform.translation + Vec3::new(0.0, CARRIED_ITEM_Y_OFFSET, 0.5);

        let icon_entity = commands
            .spawn((
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
            ))
            .id();

        commands.entity(worker_entity).try_insert(HasCarryingIndicator);

        // icon_entity は後でドロップ
        let _ = icon_entity;
    }
}

/// 運搬アイコンの位置更新とクリーンアップ
pub fn update_carrying_item_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
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
            // Inventoryがある場合のみアイコンを維持
            if let Some(item_entity) = inventory.0 {
                should_despawn = false;

                // 位置同期
                icon_transform.translation =
                    worker_transform.translation + Vec3::new(0.0, CARRIED_ITEM_Y_OFFSET, 0.5);

                // バケツの状態に応じて画像を更新
                if let Ok(item) = q_items.get(item_entity) {
                    let new_icon_handle = match item.0 {
                        ResourceType::Wood => game_assets.icon_wood_small.clone(),
                        ResourceType::Rock => game_assets.icon_rock_small.clone(),
                        ResourceType::Water => game_assets.icon_water_small.clone(),
                        ResourceType::BucketEmpty => game_assets.bucket_empty.clone(),
                        ResourceType::BucketWater => game_assets.bucket_water.clone(),
                        ResourceType::Sand => game_assets.sand_pile.clone(),
                        ResourceType::Bone => game_assets.icon_bone_small.clone(),
                        ResourceType::StasisMud => game_assets.stasis_mud.clone(),
                        ResourceType::Wheelbarrow => game_assets.icon_wheelbarrow_small.clone(),
                    };
                    // 画像が変更された場合のみ更新
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
            // HasCarryingIndicatorを削除
            if let Ok(mut entity_commands) = commands.get_entity(icon.worker) {
                entity_commands.try_remove::<HasCarryingIndicator>();
            }
        }
    }
}
