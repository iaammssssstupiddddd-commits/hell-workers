use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::logistics::{ResourceSourceKey, ResourceType};
use hw_world::{TerrainType, WorldMap};

use crate::familiar_ai::decide::task_management::validator::{
    source_key_not_reserved, source_not_reserved,
};
use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};

type TaskAssignmentQueries<'w, 's> = FamiliarTaskAssignmentQueries<'w, 's>;

pub fn find_collect_sand_source(
    target_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(ResourceSourceKey, Vec2)> {
    if let Some(best) = find_sand_pile(target_pos, task_area_opt, queries, shadow) {
        return Some(best);
    }
    if task_area_opt.is_some()
        && let Some(best) = find_sand_pile(target_pos, None, queries, shadow)
    {
        return Some(best);
    }
    if let Some(best) = scan_terrain_tiles(
        target_pos,
        task_area_opt,
        TerrainType::Sand,
        queries,
        shadow,
    ) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        return scan_terrain_tiles(target_pos, None, TerrainType::Sand, queries, shadow);
    }
    None
}

pub fn find_collect_bone_source(
    target_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(ResourceSourceKey, Vec2)> {
    if let Some(best) = find_bone_pile(target_pos, task_area_opt, queries, shadow) {
        return Some(best);
    }
    if task_area_opt.is_some()
        && let Some(best) = find_bone_pile(target_pos, None, queries, shadow)
    {
        return Some(best);
    }
    if let Some(best) = scan_terrain_tiles(
        target_pos,
        task_area_opt,
        TerrainType::River,
        queries,
        shadow,
    ) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        return scan_terrain_tiles(target_pos, None, TerrainType::River, queries, shadow);
    }
    None
}

fn find_sand_pile(
    target_pos: Vec2,
    area_filter: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(ResourceSourceKey, Vec2)> {
    queries
        .sand_piles
        .iter()
        .filter(
            |(entity, transform, designation_opt, workers_opt, pending_opt)| {
                designation_opt.is_none()
                    && workers_opt.map(|w| w.len()).unwrap_or(0) == 0
                    && pending_opt.is_none()
                    && source_not_reserved(*entity, queries, shadow)
                    && area_filter.is_none_or(|a| a.contains(transform.translation.truncate()))
            },
        )
        .min_by(|(_, t1, _, _, _), (_, t2, _, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(target_pos);
            let d2 = t2.translation.truncate().distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, transform, _, _, _)| (entity.into(), transform.translation.truncate()))
}

fn find_bone_pile(
    target_pos: Vec2,
    area_filter: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(ResourceSourceKey, Vec2)> {
    queries
        .bone_piles
        .iter()
        .filter(
            |(entity, transform, designation_opt, workers_opt, pending_opt)| {
                designation_opt.is_none()
                    && workers_opt.map(|w| w.len()).unwrap_or(0) == 0
                    && pending_opt.is_none()
                    && source_not_reserved(*entity, queries, shadow)
                    && area_filter.is_none_or(|a| a.contains(transform.translation.truncate()))
            },
        )
        .min_by(|(_, t1, _, _, _), (_, t2, _, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(target_pos);
            let d2 = t2.translation.truncate().distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, transform, _, _, _)| (entity.into(), transform.translation.truncate()))
}

fn scan_terrain_tiles(
    target_pos: Vec2,
    area_filter: Option<&TaskArea>,
    terrain_type: TerrainType,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(ResourceSourceKey, Vec2)> {
    let (x0, y0, x1, y1) = if let Some(area) = area_filter {
        let (ax0, ay0) = WorldMap::world_to_grid(area.min());
        let (ax1, ay1) = WorldMap::world_to_grid(area.max());
        (ax0, ay0, ax1, ay1)
    } else {
        (0, 0, MAP_WIDTH - 1, MAP_HEIGHT - 1)
    };

    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    let resource_type = match terrain_type {
        TerrainType::Sand => ResourceType::Sand,
        TerrainType::River => ResourceType::Bone,
        _ => return None,
    };
    let mut best: Option<(ResourceSourceKey, Vec2, f32)> = None;
    for gy in min_y..=max_y {
        for gx in min_x..=max_x {
            let Some(idx) = queries.world_map.pos_to_idx(gx, gy) else {
                continue;
            };
            if queries.world_map.terrain_at_idx(idx) != Some(terrain_type) {
                continue;
            }

            let source = ResourceSourceKey::terrain((gx, gy), resource_type);
            if !source_key_not_reserved(source, queries, shadow) {
                continue;
            }

            let tile_pos = WorldMap::grid_to_world(gx, gy);
            if let Some(area) = area_filter
                && !area.contains(tile_pos)
            {
                continue;
            }

            let dist_sq = tile_pos.distance_squared(target_pos);
            match best {
                Some((_, _, best_dist)) if best_dist <= dist_sq => {}
                _ => best = Some((source, tile_pos, dist_sq)),
            }
        }
    }

    best.map(|(entity, pos, _)| (entity, pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::ResourceReservationRequest;
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_jobs::{BonePile, DeconstructionPending};
    use hw_logistics::SharedResourceCache;
    use hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics;

    #[derive(Resource, Default)]
    struct SourceProbe(Option<ResourceSourceKey>);

    fn capture_bone_source(queries: FamiliarTaskAssignmentQueries, mut probe: ResMut<SourceProbe>) {
        probe.0 =
            find_collect_bone_source(Vec2::ZERO, None, &queries, &ReservationShadow::default())
                .map(|(source, _)| source);
    }

    #[test]
    fn pending_bone_pile_is_not_a_direct_collect_source() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .init_resource::<SourceProbe>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, capture_bone_source);

        let order = app.world_mut().spawn_empty().id();
        let pending = app
            .world_mut()
            .spawn((
                BonePile,
                Transform::from_xyz(4.0, 0.0, 0.0),
                DeconstructionPending { order },
            ))
            .id();
        let available = app
            .world_mut()
            .spawn((BonePile, Transform::from_xyz(32.0, 0.0, 0.0)))
            .id();

        app.update();

        assert_ne!(
            app.world().resource::<SourceProbe>().0,
            Some(pending.into())
        );
        assert_eq!(
            app.world().resource::<SourceProbe>().0,
            Some(available.into())
        );
    }

    #[test]
    fn river_terrain_is_collectable_without_tile_anchor_entity() {
        let mut app = App::new();
        let grid = (4, 7);
        app.init_resource::<WorldMap>();
        let idx = app
            .world()
            .resource::<WorldMap>()
            .pos_to_idx(grid.0, grid.1)
            .unwrap();
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_terrain_at_idx(idx, TerrainType::River);
        app.init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .init_resource::<SourceProbe>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, capture_bone_source);

        app.update();

        assert_eq!(
            app.world().resource::<SourceProbe>().0,
            Some(ResourceSourceKey::terrain(grid, ResourceType::Bone))
        );
        assert!(
            app.world()
                .resource::<WorldMap>()
                .tile_entities
                .iter()
                .all(Option::is_none)
        );
    }
}
