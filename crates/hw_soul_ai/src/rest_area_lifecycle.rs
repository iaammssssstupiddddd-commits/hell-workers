//! Synchronous RestArea owner cleanup used by cross-domain root transactions.

use bevy::prelude::*;
use hw_core::constants::REST_AREA_RECRUIT_COOLDOWN_SECS;
use hw_core::relationships::{RestAreaReservedFor, RestingIn};
use hw_core::soul::{IdleBehavior, IdleState, Path, RestAreaCooldown};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestAreaReleaseResult {
    Applied,
    StaleSnapshot,
}

/// Returns every Soul relationship source that references `rest_area`.
pub fn rest_area_relationship_sources(world: &mut World, rest_area: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, Option<&RestingIn>, Option<&RestAreaReservedFor>)>();
    let mut sources = query
        .iter(world)
        .filter_map(|(entity, resting, reserved)| {
            (resting.is_some_and(|relation| relation.0 == rest_area)
                || reserved.is_some_and(|relation| relation.0 == rest_area))
            .then_some(entity)
        })
        .collect::<Vec<_>>();
    sources.sort_unstable_by_key(|entity| entity.to_bits());
    sources
}

/// Releases an exact RestArea relationship snapshot and restores the same
/// visible wandering state as the normal `LeaveRestArea` lifecycle operation.
pub fn release_rest_area_for_removed_owner(
    world: &mut World,
    rest_area: Entity,
    expected_sources: &[Entity],
) -> RestAreaReleaseResult {
    if rest_area_relationship_sources(world, rest_area) != expected_sources {
        return RestAreaReleaseResult::StaleSnapshot;
    }

    for &source in expected_sources {
        let removes_resting = world
            .get::<RestingIn>(source)
            .is_some_and(|relation| relation.0 == rest_area);
        let removes_reservation = world
            .get::<RestAreaReservedFor>(source)
            .is_some_and(|relation| relation.0 == rest_area);
        if !removes_resting && !removes_reservation {
            return RestAreaReleaseResult::StaleSnapshot;
        }
    }

    for &source in expected_sources {
        let removes_resting = world
            .get::<RestingIn>(source)
            .is_some_and(|relation| relation.0 == rest_area);
        let removes_reservation = world
            .get::<RestAreaReservedFor>(source)
            .is_some_and(|relation| relation.0 == rest_area);
        let mut soul = world.entity_mut(source);
        if removes_resting {
            soul.remove::<RestingIn>();
        }
        if removes_reservation {
            soul.remove::<RestAreaReservedFor>();
        }
        soul.insert(RestAreaCooldown {
            remaining_secs: REST_AREA_RECRUIT_COOLDOWN_SECS,
        });
        if let Some(mut visibility) = soul.get_mut::<Visibility>() {
            *visibility = Visibility::Visible;
        }
        if let Some(mut idle) = soul.get_mut::<IdleState>() {
            if matches!(
                idle.behavior,
                IdleBehavior::Resting | IdleBehavior::GoingToRest
            ) {
                idle.behavior = IdleBehavior::Wandering;
            }
            idle.idle_timer = 0.0;
        }
        if let Some(mut path) = soul.get_mut::<Path>() {
            path.waypoints.clear();
            path.current_index = 0;
        }
    }
    RestAreaReleaseResult::Applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::relationships::{RestAreaOccupants, RestAreaReservations};

    #[test]
    fn exact_release_restores_occupants_and_reservations_to_visible_wandering() {
        let mut world = World::new();
        let rest_area = world.spawn_empty().id();
        let occupant = world
            .spawn((
                RestingIn(rest_area),
                IdleState {
                    behavior: IdleBehavior::Resting,
                    ..default()
                },
                Path {
                    waypoints: vec![Vec2::ONE],
                    current_index: 1,
                    ..default()
                },
                Visibility::Hidden,
            ))
            .id();
        let reserved = world
            .spawn((
                RestAreaReservedFor(rest_area),
                IdleState {
                    behavior: IdleBehavior::GoingToRest,
                    ..default()
                },
                Path::default(),
                Visibility::Visible,
            ))
            .id();
        world.flush();
        let sources = rest_area_relationship_sources(&mut world, rest_area);

        assert_eq!(
            release_rest_area_for_removed_owner(&mut world, rest_area, &sources),
            RestAreaReleaseResult::Applied
        );
        for soul in [occupant, reserved] {
            assert!(world.get::<RestingIn>(soul).is_none());
            assert!(world.get::<RestAreaReservedFor>(soul).is_none());
            assert_eq!(world.get::<Visibility>(soul), Some(&Visibility::Visible));
            assert_eq!(
                world.get::<IdleState>(soul).unwrap().behavior,
                IdleBehavior::Wandering
            );
            assert!(world.get::<Path>(soul).unwrap().waypoints.is_empty());
            assert!(world.get::<RestAreaCooldown>(soul).is_some());
        }
        assert!(world.get::<RestAreaOccupants>(rest_area).is_none());
        assert!(world.get::<RestAreaReservations>(rest_area).is_none());
    }

    #[test]
    fn changed_snapshot_is_non_mutating() {
        let mut world = World::new();
        let rest_area = world.spawn_empty().id();
        let soul = world.spawn(RestingIn(rest_area)).id();
        world.flush();

        assert_eq!(
            release_rest_area_for_removed_owner(&mut world, rest_area, &[]),
            RestAreaReleaseResult::StaleSnapshot
        );
        assert_eq!(
            world.get::<RestingIn>(soul).map(|value| value.0),
            Some(rest_area)
        );
    }
}
