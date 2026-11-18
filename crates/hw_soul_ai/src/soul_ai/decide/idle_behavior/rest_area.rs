//! 休憩所: 容量判定・候補探索・隣接歩行点計算

use std::collections::HashMap;

use bevy::prelude::*;

use hw_core::constants::TILE_SIZE;
use hw_core::relationships::{RestAreaOccupants, RestAreaReservations};
use hw_jobs::{DeconstructionPending, RestArea};
use hw_world::WorldMap;
use hw_world::coords::world_to_grid;

pub const REST_AREA_ARRIVAL_RADIUS: f32 = TILE_SIZE;

pub fn rest_area_has_capacity(
    rest_area_entity: Entity,
    rest_area: &RestArea,
    occupants: Option<&RestAreaOccupants>,
    reservations: Option<&RestAreaReservations>,
    pending_reservations: &HashMap<Entity, usize>,
) -> bool {
    let occupant_count = occupants.map_or(0, RestAreaOccupants::len);
    let reserved_count = reservations.map_or(0, RestAreaReservations::len);
    let pending_count = pending_reservations
        .get(&rest_area_entity)
        .copied()
        .unwrap_or(0);

    occupant_count + reserved_count + pending_count < rest_area.capacity
}

pub type RestAreasQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static RestArea,
        Option<&'static RestAreaOccupants>,
        Option<&'static RestAreaReservations>,
        Option<&'static DeconstructionPending>,
    ),
>;

pub(crate) fn find_nearest_available_rest_area<T>(
    pos: Vec2,
    q_rest_areas: &RestAreasQuery,
    pending_reservations: &HashMap<Entity, usize>,
    resolve_entry: impl FnMut(Vec2) -> Option<T>,
) -> Option<(Entity, T)> {
    let mut resolve_entry = resolve_entry;
    q_rest_areas
        .iter()
        .filter(
            |(rest_area_entity, _, rest_area, occupants, reservations, pending)| {
                pending.is_none()
                    && rest_area_has_capacity(
                        *rest_area_entity,
                        rest_area,
                        *occupants,
                        *reservations,
                        pending_reservations,
                    )
            },
        )
        .filter_map(|(entity, transform, ..)| {
            let center = transform.translation.truncate();
            resolve_entry(center).map(|entry| (entity, entry, center.distance_squared(pos)))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(entity, entry, _)| (entity, entry))
}

/// rest area の中心から占有グリッド 4 マスを返す。
fn rest_area_occupied_grids_from_center(center: Vec2) -> [(i32, i32); 4] {
    let top_right = WorldMap::world_to_grid(center);
    [
        (top_right.0 - 1, top_right.1 - 1),
        (top_right.0, top_right.1 - 1),
        (top_right.0 - 1, top_right.1),
        (top_right.0, top_right.1),
    ]
}

/// rest area 周辺の歩行可能候補グリッドを現在位置からの距離順で返す。
pub(crate) fn rest_area_adjacent_candidates(
    center: Vec2,
    current_pos: Vec2,
    world_map: &WorldMap,
) -> Vec<(i32, i32)> {
    let occupied = rest_area_occupied_grids_from_center(center);
    let directions: [(i32, i32); 8] = [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    let mut candidates: Vec<(i32, i32)> = occupied
        .iter()
        .flat_map(|&(gx, gy)| directions.iter().map(move |&(dx, dy)| (gx + dx, gy + dy)))
        .filter(|grid| !occupied.contains(grid))
        .filter(|&(gx, gy)| world_map.is_walkable(gx, gy))
        .collect();

    candidates.sort_unstable();
    candidates.dedup();
    candidates.sort_by(|a, b| {
        let a_pos = WorldMap::grid_to_world(a.0, a.1);
        let b_pos = WorldMap::grid_to_world(b.0, b.1);
        a_pos
            .distance_squared(current_pos)
            .partial_cmp(&b_pos.distance_squared(current_pos))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates
}

pub fn has_arrived_at_rest_area(current_pos: Vec2, rest_area_center: Vec2) -> bool {
    if current_pos.distance(rest_area_center) <= REST_AREA_ARRIVAL_RADIUS {
        return true;
    }
    let current_grid = world_to_grid(current_pos);
    let occupied = rest_area_occupied_grids_from_center(rest_area_center);
    occupied.iter().any(|&(gx, gy)| {
        let dx = (current_grid.0 - gx).abs();
        let dy = (current_grid.1 - gy).abs();
        dx <= 1 && dy <= 1 && !(dx == 0 && dy == 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct SelectedRestArea(Option<Entity>);

    fn select_nearest(q_rest_areas: RestAreasQuery, mut selected: ResMut<SelectedRestArea>) {
        selected.0 =
            find_nearest_available_rest_area(Vec2::ZERO, &q_rest_areas, &HashMap::default(), Some)
                .map(|(entity, _)| entity);
    }

    #[test]
    fn nearest_rest_area_ignores_a_pending_deconstruction_target() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SelectedRestArea>()
            .add_systems(Update, select_nearest);
        let order = app.world_mut().spawn_empty().id();
        let pending = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 0.0, 0.0),
                RestArea { capacity: 1 },
                DeconstructionPending { order },
            ))
            .id();
        let live = app
            .world_mut()
            .spawn((
                Transform::from_xyz(100.0, 0.0, 0.0),
                RestArea { capacity: 1 },
            ))
            .id();

        app.update();

        assert_ne!(pending, live);
        assert_eq!(app.world().resource::<SelectedRestArea>().0, Some(live));
    }
}
