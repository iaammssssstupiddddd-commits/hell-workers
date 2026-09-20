//! Stable identities and legal-space contract, independent of allocator Entity IDs.

use bevy::prelude::*;
use hw_jobs::BuildingType;
use hw_world::{Site, WorldMap, Yard, layout::RIVER_Y_MIN};

use super::super::PerfScenarioSize;
use crate::interface::selection::placement_geometry::building_geometry;

pub(super) const CAMERA_SCALE: f32 = 5.0;
pub(super) const KINDS: [BuildingType; 10] = [
    BuildingType::Tank,
    BuildingType::MudMixer,
    BuildingType::RestArea,
    BuildingType::SoulSpa,
    BuildingType::WheelbarrowParking,
    BuildingType::SandPile,
    BuildingType::BonePile,
    BuildingType::Door,
    BuildingType::OutdoorLamp,
    BuildingType::Bridge,
];

#[derive(Clone, Debug)]
pub(super) struct Specimen {
    pub ordinal: usize,
    pub kind: BuildingType,
    pub anchor: (i32, i32),
    pub tiles: Vec<(i32, i32)>,
    pub center: Vec2,
}

impl Specimen {
    pub fn quarter(&self) -> usize {
        self.ordinal % 4
    }

    pub fn water_count(&self) -> usize {
        [0, 25, 50, 50][self.quarter()]
    }

    pub fn rest_count(&self) -> usize {
        [0, 1, hw_core::constants::REST_AREA_CAPACITY, 0][self.quarter()]
    }

    pub fn spa_mask(&self) -> u8 {
        [0, 1, 3, 15][self.quarter()]
    }

    pub fn supports(&self) -> [(i32, i32); 2] {
        [
            (self.anchor.0 - 1, self.anchor.1),
            (self.anchor.0 + 1, self.anchor.1),
        ]
    }

    pub fn companion(&self) -> (i32, i32) {
        (self.anchor.0, self.anchor.1 + 2)
    }
}

pub(super) fn copies(size: PerfScenarioSize) -> usize {
    match size {
        PerfScenarioSize::Small => 4,
        PerfScenarioSize::Medium => 16,
        PerfScenarioSize::Large => unreachable!("building-art-static rejects large before startup"),
    }
}

pub(super) fn layout(size: PerfScenarioSize) -> Vec<Specimen> {
    KINDS
        .into_iter()
        .enumerate()
        .flat_map(|(row, kind)| {
            (0..copies(size)).map(move |ordinal| {
                let x = 8 + i32::try_from(ordinal).expect("bounded column") * 5;
                let anchor = (
                    x,
                    if kind == BuildingType::Bridge {
                        RIVER_Y_MIN
                    } else {
                        8 + i32::try_from(row).expect("bounded row") * 5
                    },
                );
                let geometry = building_geometry(kind, anchor, RIVER_Y_MIN);
                Specimen {
                    ordinal,
                    kind,
                    anchor,
                    tiles: geometry.occupied_grids,
                    center: geometry.draw_pos,
                }
            })
        })
        .collect()
}

pub(super) fn site() -> Site {
    Site {
        min: WorldMap::grid_to_world(5, 5),
        max: WorldMap::grid_to_world(88, 72),
    }
}

/// Every column has a separate grid. The first two lamps have their own unpowered
/// yard, so a one-worker Spa in column 1 cannot accidentally power its off control.
pub(super) fn yards(size: PerfScenarioSize) -> Vec<Yard> {
    (0..copies(size))
        .flat_map(|ordinal| {
            let x = 8 + i32::try_from(ordinal).expect("bounded column") * 5;
            let bottom = if ordinal % 4 < 2 { 45 } else { 50 };
            let mut yards = vec![Yard {
                min: WorldMap::grid_to_world(x - 1, 5),
                max: WorldMap::grid_to_world(x + 2, bottom),
            }];
            if ordinal % 4 < 2 {
                yards.push(Yard {
                    min: WorldMap::grid_to_world(x - 1, 47),
                    max: WorldMap::grid_to_world(x + 2, 50),
                });
            }
            yards
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn exact_inventory_and_disjoint_footprints_include_supports_and_companions() {
        for size in [PerfScenarioSize::Small, PerfScenarioSize::Medium] {
            let specs = layout(size);
            assert_eq!(specs.len(), copies(size) * 10);
            let mut cells = HashSet::new();
            for spec in &specs {
                for &(x, y) in &spec.tiles {
                    assert!((0..100).contains(&x) && (0..100).contains(&y));
                    assert!(cells.insert((x, y)), "overlap: {spec:?}");
                }
                if spec.kind == BuildingType::Tank {
                    let (x, y) = spec.companion();
                    assert!(cells.insert((x, y)) && cells.insert((x + 1, y)));
                }
                if spec.kind == BuildingType::Door {
                    for grid in spec.supports() {
                        assert!(cells.insert(grid));
                    }
                }
            }
            for kind in KINDS {
                assert_eq!(
                    specs.iter().filter(|s| s.kind == kind).count(),
                    copies(size)
                );
            }
            let actors: usize = specs
                .iter()
                .map(|s| match s.kind {
                    BuildingType::MudMixer => usize::from(s.quarter() >= 2),
                    BuildingType::RestArea => s.rest_count(),
                    BuildingType::SoulSpa => s.spa_mask().count_ones() as usize,
                    _ => 0,
                })
                .sum();
            assert_eq!(actors, copies(size) / 4 * 15);
            let yards = yards(size);
            for s in &specs {
                assert!(site().contains(s.center));
                if s.kind != BuildingType::Bridge {
                    assert_eq!(yards.iter().filter(|y| y.contains(s.center)).count(), 1);
                }
            }
        }
    }

    #[test]
    fn small_is_an_exact_subset_and_spa_uses_southward_anchor() {
        let medium = layout(PerfScenarioSize::Medium);
        for small in layout(PerfScenarioSize::Small) {
            let large = medium
                .iter()
                .find(|s| s.kind == small.kind && s.ordinal == small.ordinal)
                .unwrap();
            assert_eq!(small.tiles, large.tiles);
            assert_eq!(small.center, large.center);
            if small.kind == BuildingType::SoulSpa {
                assert_eq!(small.tiles[2], (small.anchor.0, small.anchor.1 - 1));
            }
        }
    }
}
