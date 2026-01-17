//! 運搬中アイテムのビジュアル表示システム

use bevy::prelude::*;

use super::components::{CarryingItemVisual, HasCarryingIndicator};
use super::{CARRIED_ITEM_ICON_SIZE, CARRIED_ITEM_Y_OFFSET};
use crate::assets::GameAssets;
use crate::entities::damned_soul::DamnedSoul;
use crate::relationships::Holding;
use crate::systems::logistics::{ResourceItem, ResourceType};

/// 運搬中のワーカーにアイテムアイコンを付与する
pub fn spawn_carrying_item_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_workers: Query<
        (Entity, &Transform, &Holding),
        (With<DamnedSoul>, Without<HasCarryingIndicator>),
    >,
    q_items: Query<&ResourceItem>,
) {
    for (worker_entity, transform, holding) in q_workers.iter() {
        // Holdingしているアイテムのタイプを取得
        let item_entity = holding.0;
        let Ok(item) = q_items.get(item_entity) else {
            continue;
        };

        // リソースタイプに応じたアイコンを選択
        let icon_handle = match item.0 {
            ResourceType::Wood => game_assets.icon_wood_small.clone(),
            ResourceType::Stone => game_assets.icon_stone_small.clone(),
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

        commands.entity(worker_entity).insert(HasCarryingIndicator);

        // icon_entity は後でドロップ
        let _ = icon_entity;
    }
}

/// 運搬アイコンの位置更新とクリーンアップ
pub fn update_carrying_item_system(
    mut commands: Commands,
    q_workers: Query<(Entity, &Transform, Option<&Holding>), With<DamnedSoul>>,
    mut q_icons: Query<(Entity, &CarryingItemVisual, &mut Transform), Without<DamnedSoul>>,
) {
    for (icon_entity, icon, mut icon_transform) in q_icons.iter_mut() {
        let mut should_despawn = true;

        if let Ok((_, worker_transform, holding)) = q_workers.get(icon.worker) {
            // Holdingがある場合のみアイコンを維持
            if holding.is_some() {
                should_despawn = false;

                // 位置同期
                icon_transform.translation =
                    worker_transform.translation + Vec3::new(0.0, CARRIED_ITEM_Y_OFFSET, 0.5);
            }
        }

        if should_despawn {
            info!(
                "VISUAL: Despawning carrying icon for worker {:?}",
                icon.worker
            );
            commands.entity(icon_entity).despawn();
            // HasCarryingIndicatorを削除
            if let Ok(mut entity_commands) = commands.get_entity(icon.worker) {
                entity_commands.remove::<HasCarryingIndicator>();
            }
        }
    }
}
