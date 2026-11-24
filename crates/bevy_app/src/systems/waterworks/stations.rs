use super::*;
use bevy::ecs::system::SystemParam;
use hw_logistics::{BelongsTo, BucketStorage, PendingBelongsToBlueprint, ResourceType, Stockpile};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StationProgress {
    #[default]
    Site,
    Building {
        wood: u32,
        percent: u32,
    },
    Water,
}

pub(super) fn station_inside(
    district: ContractDistrict,
    footprint: &[(i32, i32)],
    mut cells: impl Iterator<Item = (i32, i32)>,
) -> bool {
    if footprint.len() != 4 || !footprint.iter().all(|&p| district.contains(p)) {
        return false;
    }
    let (Some(a), Some(b), None) = (cells.next(), cells.next(), cells.next()) else {
        return false;
    };
    use hw_ui::selection::{TANK_NEARBY_BUCKET_STORAGE_TILES, grid_is_nearby};
    a.1 == b.1
        && a.0.abs_diff(b.0) == 1
        && [a, b].into_iter().all(|cell| {
            district.contains(cell)
                && !footprint.contains(&cell)
                && footprint
                    .iter()
                    .any(|&body| grid_is_nearby(body, cell, TANK_NEARBY_BUCKET_STORAGE_TILES))
        })
}

type StorageOwners<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        Option<&'static BelongsTo>,
        Option<&'static PendingBelongsToBlueprint>,
    ),
    With<BucketStorage>,
>;

pub(super) fn is_completed_water_tank(building: &Building, pile: &Stockpile) -> bool {
    building.kind == BuildingType::Tank
        && !building.is_provisional
        && pile.resource_type == Some(ResourceType::Water)
        && pile.capacity >= hw_core::constants::BUCKET_CAPACITY as usize
}

#[derive(SystemParam)]
pub(crate) struct Stations<'w, 's> {
    completed: Query<
        'w,
        's,
        (
            Entity,
            &'static Building,
            &'static Transform,
            &'static Stockpile,
        ),
    >,
    blueprints: Query<'w, 's, (Entity, &'static Blueprint)>,
    storage: StorageOwners<'w, 's>,
}

impl Stations<'_, '_> {
    fn has_storage(
        &self,
        district: ContractDistrict,
        id: Entity,
        footprint: &[(i32, i32)],
        pending: bool,
    ) -> bool {
        station_inside(
            district,
            footprint,
            self.storage
                .iter()
                .filter_map(|(position, owner, blueprint)| {
                    let belongs = if pending {
                        blueprint.map(|o| o.0)
                    } else {
                        owner.map(|o| o.0)
                    };
                    (belongs == Some(id))
                        .then(|| WorldMap::world_to_grid(position.translation.truncate()))
                }),
        )
    }

    fn completed_in_district(
        &self,
        district: ContractDistrict,
        (tank, building, position, pile): (Entity, &Building, &Transform, &Stockpile),
    ) -> bool {
        is_completed_water_tank(building, pile)
            && self.has_storage(district, tank, &tank_footprint(district, position), false)
    }

    pub(super) fn eligible(&self, district: ContractDistrict, tank: Entity) -> bool {
        self.completed
            .get(tank)
            .is_ok_and(|station| self.completed_in_district(district, station))
    }

    pub(crate) fn progress(&self, district: ContractDistrict) -> StationProgress {
        if self
            .completed
            .iter()
            .any(|station| self.completed_in_district(district, station))
        {
            return StationProgress::Water;
        }
        self.blueprints
            .iter()
            .filter(|(id, bp)| {
                bp.kind == BuildingType::Tank
                    && self.has_storage(district, *id, &bp.occupied_grids, true)
            })
            .map(|(_, bp)| StationProgress::Building {
                wood: bp
                    .delivered_materials
                    .get(&ResourceType::Wood)
                    .copied()
                    .unwrap_or_default(),
                percent: (bp.progress * 100.0).round() as u32,
            })
            .max()
            .unwrap_or_default()
    }
}
