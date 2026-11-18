use std::collections::HashMap;

use bevy::prelude::*;

use hw_core::gathering::{GATHERING_LEAVE_RADIUS, GatheringSpot};
use hw_core::relationships::GatheringParticipants;
use hw_spatial::{GatheringSpotSpatialGrid, SpatialGridOps};

use hw_world::{WalkabilityConnectivityCache, WorldMap};

use super::super::rest_area::{
    RestAreasQuery, find_nearest_available_rest_area, rest_area_adjacent_candidates,
};

pub(super) fn resolve_gathering_target(
    participating_in: Option<&hw_core::relationships::ParticipatingIn>,
    q_spots: &Query<(Entity, &GatheringSpot, &GatheringParticipants)>,
    spot_grid: &GatheringSpotSpatialGrid,
    transform: &Transform,
    scratch: &mut Vec<Entity>,
) -> (Option<Vec2>, Option<Entity>) {
    if let Some(p) = participating_in {
        let center = q_spots.get(p.0).ok().map(|(_, s, _)| s.center);
        (center, Some(p.0))
    } else {
        let pos = transform.translation.truncate();
        spot_grid.get_nearby_in_radius_into(pos, GATHERING_LEAVE_RADIUS * 2.0, scratch);
        let nearest = scratch
            .iter()
            .filter_map(|&e| q_spots.get(e).ok())
            .filter(|item| item.2.len() < item.1.max_capacity)
            .min_by(|a, b| {
                a.1.center
                    .distance_squared(pos)
                    .partial_cmp(&b.1.center.distance_squared(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        match nearest {
            Some((e, s, _)) => (Some(s.center), Some(e)),
            None => (None, None),
        }
    }
}

pub(super) fn resolve_rest_area_target(
    reserved_rest_area: Option<Entity>,
    previous_destination: Vec2,
    current_pos: Vec2,
    q_rest_areas: &RestAreasQuery,
    pending_rest_reservations: &HashMap<Entity, usize>,
    map: &WorldMap,
    connectivity: &mut WalkabilityConnectivityCache,
) -> Option<(Entity, Vec2)> {
    let start = WorldMap::world_to_grid(current_pos);
    let select_entry = |connectivity: &mut WalkabilityConnectivityCache, center, preferred| {
        let mut candidates = rest_area_adjacent_candidates(center, current_pos, map);
        if let Some(grid) = preferred
            && let Some(index) = candidates.iter().position(|&candidate| candidate == grid)
        {
            candidates[..=index].rotate_right(1);
        }
        candidates
            .into_iter()
            .find(|&grid| connectivity.can_reach_target(map, start, grid, true))
            .map(|(x, y)| WorldMap::grid_to_world(x, y))
    };

    if let Some(reserved_entity) = reserved_rest_area
        && let Ok((_, transform, _, _, _, None)) = q_rest_areas.get(reserved_entity)
        && let Some(entry) = select_entry(
            connectivity,
            transform.translation.truncate(),
            Some(WorldMap::world_to_grid(previous_destination)),
        )
    {
        return Some((reserved_entity, entry));
    }

    find_nearest_available_rest_area(
        previous_destination,
        q_rest_areas,
        pending_rest_reservations,
        |center| select_entry(connectivity, center, None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::RestArea;

    #[derive(Resource, Default)]
    struct Selection(Option<(Entity, Vec2)>);

    fn resolve(
        map: Res<WorldMap>,
        mut cache: ResMut<WalkabilityConnectivityCache>,
        mut selection: ResMut<Selection>,
        rest_areas: RestAreasQuery,
    ) {
        let start = WorldMap::grid_to_world(5, 11);
        selection.0 = resolve_rest_area_target(
            None,
            start,
            start,
            &rest_areas,
            &HashMap::new(),
            &map,
            &mut cache,
        );
    }

    #[test]
    fn reachable_rest_area_is_selected_even_when_the_nearest_is_isolated_test() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<Selection>()
            .add_systems(Update, resolve);
        let near = app
            .world_mut()
            .spawn((
                Transform::from_translation(WorldMap::grid_to_world(11, 11).extend(0.0)),
                RestArea { capacity: 1 },
            ))
            .id();
        let far = app
            .world_mut()
            .spawn((
                Transform::from_translation(WorldMap::grid_to_world(25, 11).extend(0.0)),
                RestArea { capacity: 1 },
            ))
            .id();
        for x in 8..=13 {
            for y in 8..=13 {
                if x == 8 || x == 13 || y == 8 || y == 13 {
                    app.world_mut()
                        .resource_mut::<WorldMap>()
                        .add_obstacle(x, y);
                }
            }
        }
        app.update();
        assert_eq!(app.world().resource::<Selection>().0.unwrap().0, far);
        app.world_mut()
            .resource_mut::<WorldMap>()
            .remove_obstacle(8, 11);
        app.update();
        let (facility, entry) = app.world().resource::<Selection>().0.unwrap();
        assert_eq!(facility, near);
        assert_eq!(WorldMap::world_to_grid(entry), (9, 11));
    }

    #[test]
    fn rest_decision_and_admission_keep_one_reachable_reservation() {
        use crate::soul_ai::decide::idle_behavior::system::idle_behavior_decision_system;
        use crate::soul_ai::execute::idle_behavior_apply::idle_behavior_apply_system;
        use crate::soul_ai::helpers::query_types::NeedsIdleDecision;
        use crate::soul_ai::update::slow_simulation::SlowSimulationClock;
        use hw_core::events::IdleBehaviorRequest;
        use hw_core::relationships::{
            RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
        };
        use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
        use hw_core::world::DoorState;
        use hw_jobs::AssignedTask;

        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<SlowSimulationClock>()
            .init_resource::<GatheringSpotSpatialGrid>()
            .init_resource::<hw_spatial::SpatialGrid>()
            .add_message::<IdleBehaviorRequest>()
            .add_systems(
                Update,
                (idle_behavior_decision_system, idle_behavior_apply_system).chain(),
            );

        let start = WorldMap::grid_to_world(5, 11);
        let souls = std::array::from_fn::<_, 2, _>(|_| {
            app.world_mut()
                .spawn((
                    DamnedSoul {
                        dream: 10.0,
                        fatigue: 1.0,
                        ..default()
                    },
                    Destination(start),
                    Transform::from_translation(start.extend(0.0)),
                    Path::default(),
                    IdleState::default(),
                    AssignedTask::None,
                    NeedsIdleDecision,
                ))
                .id()
        });
        let world = app.world_mut();
        let [near, far] = [11, 25].map(|x| {
            world
                .spawn((
                    Transform::from_translation(WorldMap::grid_to_world(x, 11).extend(0.0)),
                    RestArea { capacity: 1 },
                ))
                .id()
        });
        let near_door = world.spawn_empty().id();
        let far_door = world.spawn_empty().id();
        let mut map = world.resource_mut::<WorldMap>();
        for x in 8..=13 {
            for y in 8..=13 {
                if x == 8 || x == 13 || y == 8 || y == 13 {
                    map.add_obstacle(x, y);
                }
            }
        }
        map.register_door((8, 11), near_door, DoorState::Locked);
        for x in 23..=26 {
            for y in 9..=12 {
                if ((x == 23 || x == 26) || (y == 9 || y == 12)) && (x, y) != (23, 11) {
                    map.add_obstacle(x, y);
                }
            }
        }
        map.register_door((23, 11), far_door, DoorState::Closed);

        let reserved = |app: &App, area| {
            app.world()
                .get::<RestAreaReservations>(area)
                .map_or(0, RestAreaReservations::len)
        };
        app.update();
        assert_eq!((reserved(&app, near), reserved(&app, far)), (0, 1));
        let entrant = souls
            .into_iter()
            .find(|&soul| app.world().get::<RestAreaReservedFor>(soul).is_some())
            .unwrap();
        let entry_pos = WorldMap::grid_to_world(23, 11);
        assert_eq!(
            app.world().get::<Destination>(entrant).unwrap().0,
            entry_pos
        );

        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_door_state(23, 11, DoorState::Locked);
        app.update();
        assert_eq!((reserved(&app, near), reserved(&app, far)), (0, 0));
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_door_state(23, 11, DoorState::Closed);
        app.update();
        assert_eq!((reserved(&app, near), reserved(&app, far)), (0, 1));
        let entrant = souls
            .into_iter()
            .find(|&soul| app.world().get::<RestAreaReservedFor>(soul).is_some())
            .unwrap();
        app.world_mut()
            .get_mut::<Transform>(entrant)
            .unwrap()
            .translation = entry_pos.extend(0.0);
        app.update();

        assert_eq!(app.world().get::<RestingIn>(entrant).unwrap().0, far);
        assert_eq!(app.world().get::<RestAreaOccupants>(far).unwrap().len(), 1);
        assert_eq!((reserved(&app, near), reserved(&app, far)), (0, 0));
        assert!(app.world().get::<RestAreaOccupants>(near).is_none());
    }
}
