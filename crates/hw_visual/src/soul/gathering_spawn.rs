//! 集会スポットの visual entity 生成ヘルパー
//!
//! `GameAssets` に依存せず、`GatheringVisualHandles` を受け取ることで
//! hw_visual crate 内から呼び出せる独立 helper。

use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_AURA, Z_ITEM};
use hw_core::gathering::{
    GATHERING_GRACE_PERIOD, GATHERING_MAX_CAPACITY, GatheringObjectType, GatheringSpot,
    GatheringVisuals, calculate_aura_size,
};

use crate::handles::GatheringVisualHandles;

/// 集会スポットをスポーン（GatheringSpot + visual entities）
///
/// `GameAssets` の代わりに `GatheringVisualHandles` を受け取ることで、
/// root crate への逆依存を持たない。
pub fn spawn_gathering_spot(
    commands: &mut Commands,
    handles: &GatheringVisualHandles,
    center: Vec2,
    object_type: GatheringObjectType,
    created_at: f32,
) -> Entity {
    let spot = GatheringSpot {
        center,
        max_capacity: GATHERING_MAX_CAPACITY,
        grace_timer: GATHERING_GRACE_PERIOD,
        grace_active: true,
        object_type,
        created_at,
    };

    let aura_size = calculate_aura_size(0);

    let aura_entity = commands
        .spawn((
            Sprite {
                image: handles.aura_circle.clone(),
                custom_size: Some(Vec2::splat(aura_size)),
                color: Color::srgba(0.5, 0.2, 0.8, 0.3),
                ..default()
            },
            Transform::from_xyz(center.x, center.y, Z_AURA),
        ))
        .id();

    let object_image = match object_type {
        GatheringObjectType::Nothing => None,
        GatheringObjectType::CardTable => Some(handles.card_table.clone()),
        GatheringObjectType::Campfire => Some(handles.campfire.clone()),
        GatheringObjectType::Barrel => Some(handles.barrel.clone()),
    };
    let object_entity = object_image.map(|image| {
        commands
            .spawn((
                Sprite {
                    image,
                    custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                    ..default()
                },
                Transform::from_xyz(center.x, center.y, Z_ITEM),
            ))
            .id()
    });

    let visuals = GatheringVisuals {
        aura_entity,
        object_entity,
    };

    commands.spawn((spot, visuals)).id()
}
