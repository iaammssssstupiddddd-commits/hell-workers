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

type ResourceStackKey = ((i32, i32), ResourceType);

/// Runtime-only membership index shared by stack alpha and count-label projection.
#[derive(Resource, Default)]
pub struct ResourceStackIndex {
    entries: HashMap<Entity, Option<ResourceStackKey>>,
    members: HashMap<ResourceStackKey, HashSet<Entity>>,
    grid_counts: HashMap<(i32, i32), usize>,
    dirty_keys: HashSet<ResourceStackKey>,
    dirty_work: Vec<ResourceStackKey>,
    reset_alpha: HashSet<Entity>,
    initialized: bool,
}

impl ResourceStackIndex {
    fn remove_cached(&mut self, entity: Entity) {
        let Some(key) = self.entries.remove(&entity).flatten() else {
            self.reset_alpha.insert(entity);
            return;
        };
        self.dirty_keys.insert(key);
        self.reset_alpha.insert(entity);
        if let Some(members) = self.members.get_mut(&key) {
            members.remove(&entity);
            if members.is_empty() {
                self.members.remove(&key);
            }
        }
        if let Some(count) = self.grid_counts.get_mut(&key.0) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.grid_counts.remove(&key.0);
            }
        }
    }

    fn update_cached(
        &mut self,
        entity: Entity,
        transform: &Transform,
        visibility: &Visibility,
        item: &ResourceItem,
    ) {
        let key = matches!(visibility, Visibility::Visible | Visibility::Inherited).then(|| {
            (
                WorldMap::world_to_grid(transform.translation.truncate()),
                item.0,
            )
        });
        if self.entries.get(&entity).copied() == Some(key) {
            return;
        }
        self.remove_cached(entity);
        self.entries.insert(entity, key);
        if let Some(key) = key {
            self.members.entry(key).or_default().insert(entity);
            *self.grid_counts.entry(key.0).or_insert(0) += 1;
            self.dirty_keys.insert(key);
        }
    }
}

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
type ChangedResourceStackItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Visibility,
        &'static ResourceItem,
    ),
    Or<(
        Changed<Transform>,
        Changed<Visibility>,
        Changed<ResourceItem>,
    )>,
>;
type AddedResourceSpriteQuery<'w, 's> = Query<'w, 's, Entity, (With<ResourceItem>, Added<Sprite>)>;
type ResourceSpriteMutQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Sprite), With<ResourceItem>>;

pub fn resource_stack_display_system(
    q_items: ResourceStackItemsQuery,
    q_changed_items: ChangedResourceStackItemsQuery,
    mut removed_items: RemovedComponents<ResourceItem>,
    mut removed_transforms: RemovedComponents<Transform>,
    mut removed_visibility: RemovedComponents<Visibility>,
    mut index: ResMut<ResourceStackIndex>,
    mut sprite_queries: ParamSet<(AddedResourceSpriteQuery, ResourceSpriteMutQuery)>,
) {
    for entity in removed_items.read() {
        index.remove_cached(entity);
    }
    for entity in removed_transforms.read() {
        index.remove_cached(entity);
    }
    for entity in removed_visibility.read() {
        index.remove_cached(entity);
    }

    if index.initialized {
        for (entity, transform, visibility, item) in &q_changed_items {
            index.update_cached(entity, transform, visibility, item);
        }
    } else {
        index.initialized = true;
        for (entity, transform, visibility, item) in &q_items {
            index.update_cached(entity, transform, visibility, item);
        }
    }

    {
        let q_added_sprites = sprite_queries.p0();
        for entity in &q_added_sprites {
            index.reset_alpha.insert(entity);
            if let Some(key) = index.entries.get(&entity).copied().flatten() {
                index.dirty_keys.insert(key);
            }
        }
    }

    let mut q_item_sprites = sprite_queries.p1();
    for entity in index.reset_alpha.drain() {
        if let Ok((_, mut sprite)) = q_item_sprites.get_mut(entity)
            && sprite.color.alpha() != 1.0
        {
            sprite.color.set_alpha(1.0);
        }
    }

    let mut dirty_work = std::mem::take(&mut index.dirty_work);
    dirty_work.clear();
    dirty_work.extend(index.dirty_keys.drain());
    for key in &dirty_work {
        let Some(members) = index.members.get(key) else {
            continue;
        };
        let representative = members
            .iter()
            .min_by_key(|entity| entity.to_bits())
            .copied();
        for entity in members {
            if let Ok((_, mut sprite)) = q_item_sprites.get_mut(*entity) {
                let target_alpha = if Some(*entity) == representative {
                    1.0
                } else {
                    0.0
                };
                if sprite.color.alpha() != target_alpha {
                    sprite.color.set_alpha(target_alpha);
                }
            }
        }
    }
    index.dirty_work = dirty_work;
}

pub fn resource_count_display_system(
    mut commands: Commands,
    time: Res<Time>,
    mut refresh_timer: ResMut<ResourceCountDisplayTimer>,
    index: Res<ResourceStackIndex>,
    mut labels: ResMut<ResourceLabels>,
    mut q_text: Query<&mut Text2d, With<ResourceCountLabel>>,
    mut q_transform: Query<&mut Transform, (With<ResourceCountLabel>, Without<ResourceItem>)>,
) {
    let timer_finished = refresh_timer.timer.tick(time.delta()).just_finished();
    if refresh_timer.first_run_done && !timer_finished {
        return;
    }
    refresh_timer.first_run_done = true;

    let grid_counts = &index.grid_counts;

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
                    let next = count.to_string();
                    if text.0 != next {
                        text.0 = next;
                    }
                }
                if transform.translation != target_transform.translation
                    || transform.rotation != target_transform.rotation
                    || transform.scale != target_transform.scale
                {
                    *transform = target_transform;
                }
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
    labels.0.retain(|grid, entity| {
        if !grid_counts.contains_key(grid) {
            if let Ok(mut e) = commands.get_entity(*entity) {
                e.despawn();
            }
            false
        } else {
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_logistics::ResourceType;

    #[test]
    fn same_cell_resource_stack_draws_one_representative_without_hiding_items() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ResourceStackIndex>()
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

        let updated_items = app
            .world_mut()
            .query_filtered::<(Entity, &Sprite, &Visibility), With<ResourceItem>>()
            .iter(app.world())
            .map(|(entity, sprite, visibility)| (entity, sprite.color.alpha(), *visibility))
            .collect::<Vec<_>>();
        assert_eq!(
            updated_items
                .iter()
                .filter(|(_, alpha, visibility)| {
                    *visibility != Visibility::Hidden && *alpha == 1.0
                })
                .count(),
            1
        );
        assert_eq!(
            updated_items
                .iter()
                .filter(|(_, alpha, visibility)| {
                    *visibility != Visibility::Hidden && *alpha == 0.0
                })
                .count(),
            1
        );
        assert_eq!(
            updated_items
                .iter()
                .find(|(entity, _, _)| *entity == items[0].0)
                .map(|(_, alpha, _)| *alpha),
            Some(1.0)
        );
        assert_eq!(
            app.world()
                .resource::<ResourceStackIndex>()
                .grid_counts
                .get(&(3, 4)),
            Some(&2)
        );

        let other_center = WorldMap::grid_to_world(8, 9);
        app.world_mut()
            .entity_mut(items[1].0)
            .get_mut::<Transform>()
            .unwrap()
            .translation = other_center.extend(0.6);
        app.update();

        let index = app.world().resource::<ResourceStackIndex>();
        assert_eq!(index.grid_counts.get(&(3, 4)), Some(&1));
        assert_eq!(index.grid_counts.get(&(8, 9)), Some(&1));
        assert_eq!(
            app.world()
                .entity(items[1].0)
                .get::<Sprite>()
                .unwrap()
                .color
                .alpha(),
            1.0
        );
        assert_eq!(
            app.world()
                .entity(items[2].0)
                .get::<Sprite>()
                .unwrap()
                .color
                .alpha(),
            1.0
        );
    }
}
