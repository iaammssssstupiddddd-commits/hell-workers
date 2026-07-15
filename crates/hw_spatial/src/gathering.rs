use crate::grid::{GatheringSpotIndexTag, SpatialGridOps, SpatialIndex};
use bevy::prelude::*;
use hw_core::gathering::GatheringSpot;

/// 集会スポット用の空間グリッド
pub type GatheringSpotSpatialGrid = SpatialIndex<GatheringSpotIndexTag>;

pub fn update_gathering_spot_spatial_grid_system(
    mut grid: ResMut<GatheringSpotSpatialGrid>,
    // center はスポーン後に変化しないため Added のみで十分。
    // Changed<GatheringSpot> は grace_timer 等の毎フレーム更新で過剰発火するため除外。
    query: Query<(Entity, &GatheringSpot), Added<GatheringSpot>>,
    mut removed: RemovedComponents<GatheringSpot>,
) {
    for (entity, spot) in query.iter() {
        grid.update(entity, spot.center);
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gathering_index_uses_added_center_and_ignores_later_timer_changes() {
        let mut app = App::new();
        app.init_resource::<GatheringSpotSpatialGrid>()
            .add_systems(Update, update_gathering_spot_spatial_grid_system);

        let entity = app
            .world_mut()
            .spawn(GatheringSpot {
                center: Vec2::new(24.0, 0.0),
                ..default()
            })
            .id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<GatheringSpotSpatialGrid>()
                .get_nearby_in_radius(Vec2::new(24.0, 0.0), 1.0),
            vec![entity]
        );

        let mut entity_mut = app.world_mut().entity_mut(entity);
        let mut spot = entity_mut
            .get_mut::<GatheringSpot>()
            .expect("gathering entity has its spot component");
        spot.center = Vec2::new(96.0, 0.0);
        spot.grace_timer -= 1.0;
        app.update();

        let index = app.world().resource::<GatheringSpotSpatialGrid>();
        assert_eq!(
            index.get_nearby_in_radius(Vec2::new(24.0, 0.0), 1.0),
            vec![entity]
        );
        assert!(
            index
                .get_nearby_in_radius(Vec2::new(96.0, 0.0), 1.0)
                .is_empty()
        );
    }
}
