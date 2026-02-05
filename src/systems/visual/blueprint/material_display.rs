//! 資材アイコン・カウンター表示システム

use bevy::prelude::*;
use bevy::prelude::ChildOf;

use super::components::{MaterialCounter, MaterialIcon};
use super::{COUNTER_TEXT_OFFSET, MATERIAL_ICON_X_OFFSET, MATERIAL_ICON_Y_OFFSET};
use crate::assets::GameAssets;
use crate::systems::jobs::Blueprint;
use crate::systems::logistics::ResourceType;
use crate::systems::visual::blueprint::BlueprintVisual;

/// Blueprint に資材表示を生成する
pub fn spawn_material_display_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_blueprints: Query<(Entity, &Blueprint), (With<Blueprint>, Added<BlueprintVisual>)>,
) {
    for (bp_entity, bp) in q_blueprints.iter() {
        // BlueprintVisual が追加されたタイミングで資材表示を生成
        // 必要な資材ごとにアイコンとカウンターを生成
        let mut i = 0;
        for (resource_type, _) in &bp.required_materials {
            let icon_image = match *resource_type {
                ResourceType::Wood => game_assets.icon_wood_small.clone(),
                ResourceType::Rock => game_assets.icon_rock_small.clone(),
                ResourceType::Water => game_assets.icon_water_small.clone(),
                ResourceType::Sand => game_assets.icon_sand_small.clone(),
                ResourceType::StasisMud => game_assets.icon_stasis_mud_small.clone(),
                _ => game_assets.icon_hammer.clone(),
            };

            let offset = Vec3::new(
                MATERIAL_ICON_X_OFFSET,
                MATERIAL_ICON_Y_OFFSET - (i as f32 * 14.0),
                0.1, // 親(Z_AURA)からの相対
            );

            commands.entity(bp_entity).with_children(|parent| {
                // アイコン
                parent.spawn((
                    MaterialIcon {
                        _resource_type: *resource_type,
                    },
                    Sprite {
                        image: icon_image,
                        custom_size: Some(Vec2::splat(12.0)),
                        ..default()
                    },
                    Transform::from_translation(offset),
                    Name::new(format!("MaterialIcon ({:?})", resource_type)),
                ));

                // カウンター
                parent.spawn((
                    MaterialCounter {
                        resource_type: *resource_type,
                    },
                    Text2d::new("0/0"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Left),
                    Transform::from_translation(offset + COUNTER_TEXT_OFFSET),
                    Name::new(format!("MaterialCounter ({:?})", resource_type)),
                ));
            });

            i += 1;
        }
    }
}

/// 資材カウンターの数値を更新する
pub fn update_material_counter_system(
    q_blueprints: Query<&Blueprint>,
    mut q_counters: Query<(&MaterialCounter, &ChildOf, &mut Text2d)>,
) {
    for (counter, child_of, mut text) in q_counters.iter_mut() {
        if let Ok(bp) = q_blueprints.get(child_of.parent()) {
            let delivered = bp
                .delivered_materials
                .get(&counter.resource_type)
                .unwrap_or(&0);
            let required = bp
                .required_materials
                .get(&counter.resource_type)
                .unwrap_or(&0);
            text.0 = format!("{}/{}", delivered, required);
        }
    }
}

/// 資材アイコンとカウンターのクリーンアップ（親子関係により追従は自動）
pub fn cleanup_material_display_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, With<Blueprint>>,
    q_icons: Query<(Entity, &ChildOf, &MaterialIcon)>,
    q_counters: Query<(Entity, &ChildOf, &MaterialCounter)>,
) {
    let bp_entities: std::collections::HashSet<Entity> = q_blueprints.iter().collect();

    for (entity, child_of, _) in q_icons.iter() {
        if !bp_entities.contains(&child_of.parent()) {
            commands.entity(entity).despawn();
        }
    }

    for (entity, child_of, _) in q_counters.iter() {
        if !bp_entities.contains(&child_of.parent()) {
            commands.entity(entity).despawn();
        }
    }
}
