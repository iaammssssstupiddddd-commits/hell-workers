use super::grid::{ResourceIndexTag, SpatialGridOps, SpatialIndex};
use bevy::prelude::*;

type ResourceChangedQuery<'w, 's, T> = Query<
    'w,
    's,
    (Entity, &'static Transform, Option<&'static Visibility>),
    (
        With<T>,
        Or<(
            Added<T>,
            Added<Visibility>,
            Changed<Transform>,
            Changed<Visibility>,
        )>,
    ),
>;

/// リソースアイテム用の空間グリッド
pub type ResourceSpatialGrid = SpatialIndex<ResourceIndexTag>;

pub fn update_resource_spatial_grid_system<T: Component>(
    mut grid: ResMut<ResourceSpatialGrid>,
    q_changed: ResourceChangedQuery<T>,
    q_resource_transform: Query<&Transform, With<T>>,
    mut removed_items: RemovedComponents<T>,
    mut removed_visibility: RemovedComponents<Visibility>,
) {
    // 変更があったエンティティのみ更新（移動・表示切替）
    for (entity, transform, visibility) in q_changed.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            grid.update(entity, transform.translation.truncate());
        } else {
            // 非表示になった場合はグリッドから削除
            grid.remove(entity);
        }
    }

    // Visibility コンポーネントが外れた場合は可視扱いで再登録する。
    for entity in removed_visibility.read() {
        if let Ok(transform) = q_resource_transform.get(entity) {
            grid.update(entity, transform.translation.truncate());
        }
    }

    // 削除されたアイテムをグリッドから除去
    for entity in removed_items.read() {
        grid.remove(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct TestResource;

    #[test]
    fn visibility_policy_excludes_hidden_and_reregisters_when_visibility_is_removed() {
        let mut app = App::new();
        app.init_resource::<ResourceSpatialGrid>()
            .add_systems(Update, update_resource_spatial_grid_system::<TestResource>);

        let entity = app
            .world_mut()
            .spawn((
                TestResource,
                Transform::from_xyz(32.0, 0.0, 0.0),
                Visibility::Hidden,
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(32.0, 0.0), 1.0)
                .is_empty()
        );

        app.world_mut()
            .entity_mut(entity)
            .insert(Visibility::Visible);
        app.update();
        assert_eq!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(32.0, 0.0), 1.0),
            vec![entity]
        );

        app.world_mut()
            .entity_mut(entity)
            .insert(Visibility::Hidden);
        app.update();
        assert!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(32.0, 0.0), 1.0)
                .is_empty()
        );

        app.world_mut().entity_mut(entity).remove::<Visibility>();
        app.update();
        assert_eq!(
            app.world()
                .resource::<ResourceSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(32.0, 0.0), 1.0),
            vec![entity]
        );
    }
}
