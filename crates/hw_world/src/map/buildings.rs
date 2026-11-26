use super::WorldMap;
use bevy::prelude::*;
use hw_core::world::DoorState;
use hw_jobs::BuildingType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccupancyConflict {
    pub grid: (i32, i32),
    pub expected: Entity,
    pub actual: Option<Entity>,
}

impl WorldMap {
    /// Gathering props can extend into neighboring cells even when their center is outdoors.
    pub fn fits_gathering_prop(&self, center: Vec2) -> bool {
        let half = Vec2::splat(hw_core::gathering::GATHERING_OBJECT_SIZE * 0.5);
        let (x0, y0) = Self::world_to_grid(center - half);
        let (x1, y1) = Self::world_to_grid(center + half);
        (x0..=x1).all(|x| {
            (y0..=y1).all(|y| {
                let grid = (x, y);
                let cell_min =
                    Self::grid_to_world(x, y) - Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
                cell_min.x >= center.x + half.x
                    || cell_min.y >= center.y + half.y
                    || (self.is_walkable(x, y)
                        && !self.has_building(grid)
                        && self.floor_entity(grid).is_none()
                        && self.door_entity(x, y).is_none()
                        && !self.has_stockpile(grid))
            })
        })
    }

    pub fn building_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.buildings.get(&grid).copied()
    }

    pub fn has_building(&self, grid: (i32, i32)) -> bool {
        self.buildings.contains_key(&grid)
    }

    pub fn set_building(&mut self, grid: (i32, i32), entity: Entity) {
        self.buildings.insert(grid, entity);
    }

    pub fn clear_building(&mut self, grid: (i32, i32)) -> Option<Entity> {
        self.buildings.remove(&grid)
    }

    /// Clears only the owner-bearing building layer when it still belongs to
    /// `entity`. Passable facilities such as Soul Spa and Outdoor Lamp use
    /// this path so their removal never erases an unrelated raw obstacle bit.
    pub fn clear_building_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
        if self.building_entity(grid) != Some(entity) {
            return false;
        }
        self.clear_building(grid);
        true
    }

    pub fn set_building_occupancy(&mut self, grid: (i32, i32), entity: Entity) {
        self.set_building(grid, entity);
        self.add_obstacle(grid.0, grid.1);
        // add_obstacle が version を更新する。建物登録のみの変更はここでは追加しない。
    }

    pub fn set_building_occupancies<I>(&mut self, entity: Entity, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            self.set_building_occupancy(grid, entity);
        }
    }

    pub fn clear_building_occupancy(&mut self, grid: (i32, i32)) -> Option<Entity> {
        let entity = self.clear_building(grid);
        self.remove_obstacle(grid.0, grid.1);
        entity
    }

    pub fn clear_building_occupancy_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
        if !self.clear_building_if_owned(grid, entity) {
            return false;
        }
        self.remove_obstacle(grid.0, grid.1);
        true
    }

    pub fn release_building_grid_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
        self.clear_building_occupancy_if_owned(grid, entity)
    }

    /// Validate the entire footprint before callers change either ECS or map state.
    pub fn validate_owned_footprint<I>(
        &self,
        entity: Entity,
        grids: I,
    ) -> Result<(), OccupancyConflict>
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            let actual = self.building_entity(grid);
            if actual != Some(entity) {
                return Err(OccupancyConflict {
                    grid,
                    expected: entity,
                    actual,
                });
            }
        }
        Ok(())
    }

    /// An ownership conflict never releases even the matching subset.
    pub fn release_building_footprint_if_owned<I>(&mut self, entity: Entity, grids: I) -> bool
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        let grids: Vec<_> = grids.into_iter().collect();
        if self
            .validate_owned_footprint(entity, grids.iter().copied())
            .is_err()
        {
            return false;
        }
        for grid in grids {
            self.clear_building_occupancy(grid);
        }
        true
    }

    pub fn building_entries(&self) -> impl Iterator<Item = (&(i32, i32), &Entity)> {
        self.buildings.iter()
    }

    pub fn footprint_owner(&self, kind: BuildingType, grid: (i32, i32)) -> Option<Entity> {
        if kind == BuildingType::Equipment(hw_jobs::Equipment::Conduit) {
            self.conduits.get(&grid).copied()
        } else {
            self.building_entity(grid)
        }
    }

    pub fn reserve_building_footprint<I>(
        &mut self,
        building_type: BuildingType,
        entity: Entity,
        grids: I,
    ) where
        I: IntoIterator<Item = (i32, i32)>,
    {
        match building_type {
            BuildingType::Equipment(hw_jobs::Equipment::Conduit) => {
                for grid in grids {
                    self.conduits.insert(grid, entity);
                }
            }
            BuildingType::Bridge => {
                for grid in grids {
                    self.set_building(grid, entity);
                }
            }
            _ => self.set_building_occupancies(entity, grids),
        }
    }

    pub fn register_completed_building_footprint<I>(
        &mut self,
        building_type: BuildingType,
        entity: Entity,
        grids: I,
    ) where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.register_completed_footprint(building_type, entity, grids, &Default::default());
    }

    /// Atomically transfer a verified Blueprint footprint. The caller supplies
    /// live obstacle sources that survive removal of its placement markers.
    pub fn complete_owned_building_footprint(
        &mut self,
        expected: Entity,
        entity: Entity,
        building_type: BuildingType,
        grids: &[(i32, i32)],
        retained_obstacles: &std::collections::HashSet<(i32, i32)>,
    ) -> Result<(), OccupancyConflict> {
        for &grid in grids {
            let actual = self.footprint_owner(building_type, grid);
            if actual != Some(expected) {
                return Err(OccupancyConflict {
                    grid,
                    expected,
                    actual,
                });
            }
        }
        self.register_completed_footprint(
            building_type,
            entity,
            grids.iter().copied(),
            retained_obstacles,
        );
        Ok(())
    }

    fn register_completed_footprint<I>(
        &mut self,
        building_type: BuildingType,
        entity: Entity,
        grids: I,
        retained_obstacles: &std::collections::HashSet<(i32, i32)>,
    ) where
        I: IntoIterator<Item = (i32, i32)>,
    {
        match building_type {
            BuildingType::Equipment(hw_jobs::Equipment::Conduit) => {
                for grid in grids {
                    self.conduits.insert(grid, entity);
                }
            }
            BuildingType::Bridge => {
                for grid in grids {
                    self.register_bridge_tile(grid, entity);
                }
            }
            BuildingType::Door => {
                for grid in grids {
                    self.register_door(grid, entity, DoorState::Closed);
                }
            }
            kind if kind.blocks_movement() => self.set_building_occupancies(entity, grids),
            _ => {
                for grid in grids {
                    // Every non-Bridge Blueprint reserves a raw obstacle while
                    // it exists. Passable completed buildings retain logical
                    // occupancy but must release that placement reservation.
                    if !retained_obstacles.contains(&grid) {
                        self.remove_grid_obstacle(grid);
                    }
                    self.set_building(grid, entity);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gathering_prop_needs_clear_ground_across_its_whole_sprite() {
        let mut map = WorldMap::default();
        let owner = Entity::from_bits(42);
        let center = WorldMap::grid_to_world(12, 12);
        assert!(map.fits_gathering_prop(center));
        for kind in [BuildingType::Floor, BuildingType::Wall, BuildingType::Door] {
            let mut occupied = WorldMap::default();
            occupied.register_completed_building_footprint(kind, owner, [(13, 13)]);
            assert!(!occupied.fits_gathering_prop(center));
        }
        map.add_obstacle(11, 12);
        assert!(!map.fits_gathering_prop(center));
        assert!(map.fits_gathering_prop(center + Vec2::X * hw_core::constants::TILE_SIZE * 0.25));
        assert!(!map.fits_gathering_prop(WorldMap::grid_to_world(0, 0)));
    }

    #[test]
    fn owned_footprint_release_is_atomic_and_mismatch_preserves_all_layers_test() {
        let mut map = WorldMap::default();
        let owner = Entity::from_bits(1);
        let other = Entity::from_bits(2);
        map.set_building_occupancies(owner, [(20, 20), (21, 20)]);
        map.set_building((21, 20), other);
        map.register_door((22, 20), other, DoorState::Locked);
        map.register_bridge_tile((23, 20), other);
        let layers = (
            map.buildings.clone(),
            map.obstacles.clone(),
            map.doors.clone(),
            map.door_states.clone(),
            map.bridged_tiles.clone(),
            map.obstacle_version,
        );
        assert!(!map.release_building_footprint_if_owned(owner, [(20, 20), (21, 20)]));
        for grid in [(21, 20), (22, 20), (23, 20), (24, 20)] {
            assert!(!map.release_building_grid_if_owned(grid, owner));
        }
        assert_eq!(
            layers,
            (
                map.buildings.clone(),
                map.obstacles.clone(),
                map.doors.clone(),
                map.door_states.clone(),
                map.bridged_tiles.clone(),
                map.obstacle_version
            )
        );
        map.set_building((21, 20), owner);
        assert!(map.release_building_footprint_if_owned(owner, [(20, 20), (21, 20)]));
        assert!(map.is_walkable(20, 20));
        assert!(map.is_walkable(21, 20));
    }

    #[test]
    fn completion_transfer_preserves_other_sources_and_rejects_stale_owner_test() {
        let mut map = WorldMap::default();
        let blueprint = Entity::from_bits(1);
        let building = Entity::from_bits(2);
        let grids = [(30, 30), (31, 30)];
        map.reserve_building_footprint(BuildingType::OutdoorLamp, blueprint, grids);
        let before = map.obstacle_version;
        map.complete_owned_building_footprint(
            blueprint,
            building,
            BuildingType::OutdoorLamp,
            &grids,
            &grids.into_iter().collect(),
        )
        .unwrap();
        assert_eq!(map.obstacle_version, before);
        assert!(map.has_raw_obstacle(30, 30));
        assert!(!map.release_building_footprint_if_owned(blueprint, grids));
        assert!(
            map.complete_owned_building_footprint(
                blueprint,
                Entity::from_bits(3),
                BuildingType::OutdoorLamp,
                &grids,
                &Default::default()
            )
            .is_err()
        );
        assert_eq!(map.building_entity(grids[0]), Some(building));
        assert_eq!(map.obstacle_version, before);
    }

    #[test]
    fn passable_completion_transfers_owner_and_releases_blueprint_obstacle() {
        let mut map = WorldMap::default();
        let grid = (12, 13);
        let blueprint = Entity::from_bits(1);
        let building = Entity::from_bits(2);
        map.reserve_building_footprint(BuildingType::OutdoorLamp, blueprint, [grid]);
        assert_eq!(map.building_entity(grid), Some(blueprint));
        assert!(map.has_raw_obstacle(grid.0, grid.1));

        map.register_completed_building_footprint(BuildingType::OutdoorLamp, building, [grid]);

        assert_eq!(map.building_entity(grid), Some(building));
        assert!(!map.has_raw_obstacle(grid.0, grid.1));
        assert!(map.is_walkable(grid.0, grid.1));
    }

    #[test]
    fn owner_safe_passable_clear_preserves_an_unrelated_raw_obstacle() {
        let mut map = WorldMap::default();
        let grid = (14, 15);
        let owner = Entity::from_bits(3);
        let replacement = Entity::from_bits(4);
        map.set_building(grid, replacement);
        map.add_grid_obstacle(grid);
        let version = map.obstacle_version;

        assert!(!map.clear_building_if_owned(grid, owner));
        assert_eq!(map.building_entity(grid), Some(replacement));
        assert!(map.has_raw_obstacle(grid.0, grid.1));
        assert_eq!(map.obstacle_version, version);

        map.set_building(grid, owner);
        assert!(map.clear_building_if_owned(grid, owner));
        assert_eq!(map.building_entity(grid), None);
        assert!(map.has_raw_obstacle(grid.0, grid.1));
        assert!(!map.is_walkable(grid.0, grid.1));
        assert_eq!(map.obstacle_version, version);
    }
}

#[cfg(test)]
mod conduit_tests {
    use super::*;
    #[test]
    fn underground_blueprint_and_completion_preserve_wall_and_floor_ownership() {
        let mut map = WorldMap::default();
        let grid = (20, 20);
        let floor = Entity::from_bits(1);
        let wall = Entity::from_bits(2);
        let plan = Entity::from_bits(3);
        let pipe = Entity::from_bits(4);
        let kind = BuildingType::Equipment(hw_jobs::Equipment::Conduit);
        map.set_floor(grid, floor);
        map.set_building_occupancy(grid, wall);
        let version = map.obstacle_version;
        map.reserve_building_footprint(kind, plan, [grid]);
        assert!(
            map.complete_owned_building_footprint(wall, pipe, kind, &[grid], &Default::default())
                .is_err()
        );
        map.complete_owned_building_footprint(plan, pipe, kind, &[grid], &Default::default())
            .unwrap();
        assert_eq!(map.conduits.remove(&grid), Some(pipe));
        assert_eq!(map.building_entity(grid), Some(wall));
        assert_eq!(map.floor_entity(grid), Some(floor));
        assert_eq!(map.obstacle_version, version);
        assert!(map.has_raw_obstacle(grid.0, grid.1));
    }
}
