use super::{ResourceItem, ResourceType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;
use std::collections::{HashMap, HashSet};

#[derive(Resource, Default)]
pub struct ResourceLabels(pub HashMap<(i32, i32), Entity>);

#[derive(Resource)]
pub struct ResourceCountDisplayTimer {
    timer: Timer,
    first_run_done: bool,
}

impl Default for ResourceCountDisplayTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.25, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

#[derive(Component)]
pub struct ResourceCountLabel;

type ResourceStackItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Visibility,
        &'static ResourceItem,
    ),
>;

pub fn resource_stack_display_system(
    q_items: ResourceStackItemsQuery,
    mut q_item_sprites: Query<(Entity, &mut Sprite), With<ResourceItem>>,
) {
    let mut visible_items = HashSet::new();
    let mut stack_representatives: HashMap<((i32, i32), ResourceType), Entity> = HashMap::new();

    for (entity, transform, visibility, item) in q_items.iter() {
        if matches!(visibility, Visibility::Visible | Visibility::Inherited) {
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            visible_items.insert(entity);
            stack_representatives
                .entry((grid, item.0))
                .and_modify(|representative| {
                    if entity.to_bits() < representative.to_bits() {
                        *representative = entity;
                    }
                })
                .or_insert(entity);
        }
    }

    let visible_representatives: HashSet<_> = stack_representatives.into_values().collect();
    for (entity, mut sprite) in &mut q_item_sprites {
        let target_alpha =
            if visible_items.contains(&entity) && !visible_representatives.contains(&entity) {
                0.0
            } else {
                1.0
            };
        if sprite.color.alpha() != target_alpha {
            sprite.color.set_alpha(target_alpha);
        }
    }
}

pub fn resource_count_display_system(
    mut commands: Commands,
    time: Res<Time>,
    mut refresh_timer: ResMut<ResourceCountDisplayTimer>,
    q_items: Query<(&Transform, &Visibility), With<ResourceItem>>,
    mut labels: ResMut<ResourceLabels>,
    mut q_text: Query<&mut Text2d, With<ResourceCountLabel>>,
    mut q_transform: Query<&mut Transform, (With<ResourceCountLabel>, Without<ResourceItem>)>,
) {
    let mut grid_counts: HashMap<(i32, i32), usize> = HashMap::new();

    let timer_finished = refresh_timer.timer.tick(time.delta()).just_finished();
    if refresh_timer.first_run_done && !timer_finished {
        return;
    }
    refresh_timer.first_run_done = true;

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
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Center),
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

#[cfg(test)]
mod tests {
    use super::*;
    use hw_logistics::ResourceType;

    #[test]
    fn same_cell_resource_stack_draws_one_representative_without_hiding_items() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, resource_stack_display_system);
        let center = WorldMap::grid_to_world(3, 4);
        for offset_x in [-4.0, 0.0, 4.0] {
            app.world_mut().spawn((
                ResourceItem(ResourceType::Rock),
                Sprite::default(),
                Transform::from_translation((center + Vec2::new(offset_x, 0.0)).extend(0.6)),
            ));
        }

        app.update();

        let mut items = app
            .world_mut()
            .query_filtered::<(Entity, &Sprite, &Visibility), With<ResourceItem>>()
            .iter(app.world())
            .map(|(entity, sprite, visibility)| (entity, sprite.color.alpha(), *visibility))
            .collect::<Vec<_>>();
        items.sort_unstable_by_key(|(entity, _, _)| entity.to_bits());
        assert_eq!(
            items.iter().filter(|(_, alpha, _)| *alpha == 1.0).count(),
            1
        );
        assert_eq!(
            items.iter().filter(|(_, alpha, _)| *alpha == 0.0).count(),
            2
        );
        assert!(
            items
                .iter()
                .all(|(_, _, visibility)| *visibility != Visibility::Hidden)
        );

        app.world_mut()
            .entity_mut(items[0].0)
            .insert(Visibility::Hidden);
        app.update();

        let visible_alphas = app
            .world_mut()
            .query_filtered::<(&Sprite, &Visibility), With<ResourceItem>>()
            .iter(app.world())
            .filter_map(|(sprite, visibility)| {
                (*visibility != Visibility::Hidden).then_some(sprite.color.alpha())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            visible_alphas.iter().filter(|alpha| **alpha == 1.0).count(),
            1
        );
        assert_eq!(
            visible_alphas.iter().filter(|alpha| **alpha == 0.0).count(),
            1
        );
    }
}
