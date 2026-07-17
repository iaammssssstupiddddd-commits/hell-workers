use std::collections::HashMap;

use bevy::prelude::*;

use hw_core::gathering::{GATHERING_LEAVE_RADIUS, GatheringSpot};
use hw_core::relationships::GatheringParticipants;
use hw_spatial::{GatheringSpotSpatialGrid, SpatialGridOps};

use super::super::rest_area::{RestAreasQuery, find_nearest_available_rest_area};

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
    pos_a: Vec2,
    pos_b: Vec2,
    q_rest_areas: &RestAreasQuery,
    pending_rest_reservations: &HashMap<Entity, usize>,
) -> Option<(Entity, Vec2)> {
    reserved_rest_area
        .and_then(|reserved_entity| {
            q_rest_areas
                .get(reserved_entity)
                .ok()
                .map(|(_, t, _, _, _)| (reserved_entity, t.translation.truncate()))
        })
        .or_else(|| {
            find_nearest_available_rest_area(pos_a, q_rest_areas, pending_rest_reservations)
        })
        .or_else(|| {
            find_nearest_available_rest_area(pos_b, q_rest_areas, pending_rest_reservations)
        })
}
