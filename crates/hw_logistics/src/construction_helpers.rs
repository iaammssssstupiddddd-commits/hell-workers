//! 建設キャンセル時の共通ヘルパー。

use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_ITEM_PICKUP};
use crate::types::{ResourceItem, ResourceType};

/// bevy_app から注入されるリソースアイテム系ビジュアルアセットハンドル。
#[derive(Resource)]
pub struct ResourceItemVisualHandles {
    pub icon_bone_small: Handle<Image>,
    pub icon_wood_small: Handle<Image>,
    pub icon_stasis_mud_small: Handle<Image>,
}

/// `center` 周辺にリファンドアイテムエンティティをスポーンする。
///
/// 建設キャンセル時に未完成の資材をワールドに返却するために使用する。
pub fn spawn_refund_items(
    commands: &mut Commands,
    handles: &ResourceItemVisualHandles,
    center: Vec2,
    resource_type: ResourceType,
    amount: u32,
) {
    if amount == 0 {
        return;
    }

    let (image, name) = match resource_type {
        ResourceType::Bone => (handles.icon_bone_small.clone(), "Item (Bone, Refund)"),
        ResourceType::Wood => (handles.icon_wood_small.clone(), "Item (Wood, Refund)"),
        ResourceType::StasisMud => (
            handles.icon_stasis_mud_small.clone(),
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
