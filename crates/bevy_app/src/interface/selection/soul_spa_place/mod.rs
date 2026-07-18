//! Soul Spa 2×2 配置システム

mod input;
mod spawn;

pub use input::soul_spa_place_input_system;

use crate::systems::jobs::BuildingType;
use crate::world::map::{RIVER_Y_MIN, WorldMap, WorldMapRef};
use hw_ui::selection::{
    BuildingPlacementContext, PlacementGeometry, PlacementValidation, building_geometry,
    validate_building_placement,
};

pub(crate) fn validate_soul_spa_placement(
    world_map: &WorldMap,
    anchor: (i32, i32),
    footprint_in_yard: bool,
) -> (PlacementGeometry, PlacementValidation) {
    let geometry = building_geometry(BuildingType::SoulSpa, anchor, RIVER_Y_MIN);
    let read_world = WorldMapRef(world_map);
    let context = BuildingPlacementContext {
        world: &read_world,
        in_site: true,
        in_yard: footprint_in_yard,
        is_wall_or_door_at: &|_| false,
        is_replaceable_wall_at: &|_| false,
    };
    let validation =
        validate_building_placement(&context, BuildingType::SoulSpa, anchor, &geometry);
    (geometry, validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Entity;
    use hw_ui::selection::PlacementRejectReason;

    #[test]
    fn soul_spa_placement_shared_validator_covers_yard_occupancy_and_bounds() {
        let mut world_map = WorldMap::default();
        let anchor = (10, 10);

        let (_, valid) = validate_soul_spa_placement(&world_map, anchor, true);
        assert!(valid.can_place);

        let (_, outside_yard) = validate_soul_spa_placement(&world_map, anchor, false);
        assert_eq!(
            outside_yard.reject_reason,
            Some(PlacementRejectReason::NotInYard)
        );

        world_map.set_building(anchor, Entity::from_bits(1));
        let (_, occupied) = validate_soul_spa_placement(&world_map, anchor, true);
        assert_eq!(
            occupied.reject_reason,
            Some(PlacementRejectReason::OccupiedByBuilding)
        );

        let (_, out_of_bounds) = validate_soul_spa_placement(&world_map, (-1, -1), true);
        assert_eq!(
            out_of_bounds.reject_reason,
            Some(PlacementRejectReason::OutOfBounds)
        );
    }
}
