use super::types::ResourceItem;
use crate::constants::*;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource, Default)]
pub struct ResourceLabels(pub HashMap<(i32, i32), Entity>);

#[derive(Component)]
pub struct ResourceCountLabel;

pub fn resource_count_display_system(
    mut commands: Commands,
    q_items: Query<(&Transform, &Visibility), With<ResourceItem>>,
    mut labels: ResMut<ResourceLabels>,
    mut q_text: Query<&mut Text2d, With<ResourceCountLabel>>,
    mut q_transform: Query<&mut Transform, (With<ResourceCountLabel>, Without<ResourceItem>)>,
) {
    let mut grid_counts: HashMap<(i32, i32), usize> = HashMap::new();

    for (transform, visibility) in q_items.iter() {
        if matches!(visibility, Visibility::Visible | Visibility::Inherited) {
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            *grid_counts.entry(grid).or_insert(0) += 1;
        }
    }

    // ラベルの更新または作成
    for (grid, count) in grid_counts.iter() {
        let pos = WorldMap::grid_to_world(grid.0, grid.1);
        // 新しい座標系では pos は中心なので、右上端 (32*0.5=16) 寄りにオフセット
        // 0.35 * 32 = 11.2 なので正確にタイルの内側に収まる
        let target_transform = Transform::from_xyz(
            pos.x + TILE_SIZE * 0.35,
            pos.y + TILE_SIZE * 0.35,
            Z_CHARACTER,
        );

        if let Some(&entity) = labels.0.get(grid) {
            if let Ok(mut transform) = q_transform.get_mut(entity) {
                if let Ok(mut text) = q_text.get_mut(entity) {
                    text.0 = count.to_string();
                }
                *transform = target_transform;
            } else {
                // エンティティが存在しないか、Transformを持っていない場合は再作成フラグ
                labels.0.remove(grid);
            }
        }

        // 存在しない、または上記で remove された場合は作成
        if !labels.0.contains_key(grid) {
            let entity = commands
                .spawn((
                    ResourceCountLabel,
                    Text2d::new(count.to_string()),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Center),
                    target_transform,
                ))
                .id();
            labels.0.insert(*grid, entity);
        }
    }

    // 不要なラベルの削除
    let mut to_remove = Vec::new();
    for (&grid, &entity) in labels.0.iter() {
        if !grid_counts.contains_key(&grid) {
            if let Ok(mut e) = commands.get_entity(entity) {
                e.despawn();
            }
            to_remove.push(grid);
        }
    }
    for grid in to_remove {
        labels.0.remove(&grid);
    }
}
