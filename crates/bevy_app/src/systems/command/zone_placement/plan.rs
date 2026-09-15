//! Shared, side-effect-free zone decisions for preview and commit.
use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, YARD_MIN_HEIGHT_TILES, YARD_MIN_WIDTH_TILES};
use hw_core::{WorldEpoch, game_state::TaskModeZoneType};
use hw_world::zones::{AreaBounds, Site, Yard};
use hw_world::{
    WorldMap, area_tile_size, expand_yard_area, rectangles_overlap, rectangles_overlap_site,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneReject {
    EmptyArea,
    OutsideMap,
    OutsideYard,
    Occupied,
    NotWalkable,
    TooSmall,
    SiteOverlap,
    YardOverlap,
    Unchanged,
    PreviewChanged,
}

impl ZoneReject {
    pub fn label(self) -> &'static str {
        match self {
            Self::EmptyArea => "範囲がありません",
            Self::OutsideMap => "マップの範囲外です",
            Self::OutsideYard => "Yardの外側を含んでいます",
            Self::Occupied => "建物またはStockpileが占有しています",
            Self::NotWalkable => "歩行できないセルです",
            Self::TooSmall => "Yardの最小寸法を満たしていません",
            Self::SiteOverlap => "Siteと重なっています",
            Self::YardOverlap => "別のYardと重なっています",
            Self::Unchanged => "変更する範囲がありません",
            Self::PreviewChanged => "プレビューの条件が変わりました。範囲を指定し直してください",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ZonePlan {
    Stockpile {
        cells: Vec<((i32, i32), Entity)>,
        skipped: Vec<((i32, i32), ZoneReject)>,
    },
    Yard {
        owner: Entity,
        previous: AreaBounds,
        bounds: AreaBounds,
        added_tiles: usize,
    },
}

impl ZonePlan {
    pub fn summary(&self) -> String {
        match self {
            Self::Stockpile { cells, skipped } => {
                let reason = skipped.first().map_or("", |(_, reason)| reason.label());
                format!("採用 {} / 除外 {} {reason}", cells.len(), skipped.len())
            }
            Self::Yard { added_tiles, .. } => format!("Yard拡張: 追加 {added_tiles} セル"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZonePreview {
    pub epoch: WorldEpoch,
    pub kind: TaskModeZoneType,
    pub start: Vec2,
    pub area: AreaBounds,
    pub result: Result<ZonePlan, ZoneReject>,
}

#[derive(Resource, Default)]
pub struct ZonePlacementPreview(pub Option<ZonePreview>);

pub fn build_zone_plan(
    kind: TaskModeZoneType,
    start: Vec2,
    area: &AreaBounds,
    world: &WorldMap,
    yards: &[(Entity, Yard)],
    sites: &[Site],
) -> Result<ZonePlan, ZoneReject> {
    if area.min.x >= area.max.x || area.min.y >= area.max.y {
        return Err(ZoneReject::EmptyArea);
    }
    let min = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));
    if min.0 < 0 || min.1 < 0 || max.0 >= MAP_WIDTH || max.1 >= MAP_HEIGHT {
        return Err(ZoneReject::OutsideMap);
    }
    match kind {
        TaskModeZoneType::Stockpile => {
            let mut cells = Vec::new();
            let mut skipped = Vec::new();
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    let owner = yards
                        .iter()
                        .filter(|(_, yard)| yard.contains(WorldMap::grid_to_world(x, y)))
                        .map(|(entity, _)| *entity)
                        .min_by_key(|entity| entity.to_bits())
                        .ok_or(ZoneReject::OutsideYard)?;
                    let grid = (x, y);
                    if world.has_stockpile(grid) || world.has_building(grid) {
                        skipped.push((grid, ZoneReject::Occupied));
                    } else if !world.is_walkable(x, y) {
                        skipped.push((grid, ZoneReject::NotWalkable));
                    } else {
                        cells.push((grid, owner));
                    }
                }
            }
            if cells.is_empty() {
                return Err(skipped
                    .first()
                    .map_or(ZoneReject::EmptyArea, |(_, reason)| *reason));
            }
            Ok(ZonePlan::Stockpile { cells, skipped })
        }
        TaskModeZoneType::Yard => {
            let (owner, yard) = yards
                .iter()
                .filter(|(_, yard)| yard.contains(start))
                .min_by_key(|(entity, _)| entity.to_bits())
                .ok_or(ZoneReject::OutsideYard)?;
            let bounds = expand_yard_area(yard, area);
            let size = area_tile_size(&bounds);
            if size.0 < YARD_MIN_WIDTH_TILES as usize || size.1 < YARD_MIN_HEIGHT_TILES as usize {
                return Err(ZoneReject::TooSmall);
            }
            if sites
                .iter()
                .any(|site| rectangles_overlap_site(&bounds, site))
            {
                return Err(ZoneReject::SiteOverlap);
            }
            if yards
                .iter()
                .any(|(entity, other)| entity != owner && rectangles_overlap(&bounds, other))
            {
                return Err(ZoneReject::YardOverlap);
            }
            let previous = yard.bounds();
            if bounds == previous {
                return Err(ZoneReject::Unchanged);
            }
            let old = area_tile_size(&previous);
            Ok(ZonePlan::Yard {
                owner: *owner,
                previous,
                bounds,
                added_tiles: (size.0 * size.1).saturating_sub(old.0 * old.1),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::constants::TILE_SIZE;

    fn bounds(min: (i32, i32), max: (i32, i32)) -> AreaBounds {
        let half = Vec2::splat(TILE_SIZE * 0.5);
        AreaBounds::from_points(
            WorldMap::grid_to_world(min.0, min.1) - half,
            WorldMap::grid_to_world(max.0, max.1) + half,
        )
    }

    #[test]
    fn stockpile_plan_counts_skips_and_keeps_per_cell_yard_owners() {
        let mut world = WorldMap::default();
        let mut entities = World::new();
        let first = entities.spawn_empty().id();
        let second = entities.spawn_empty().id();
        let left = bounds((10, 10), (11, 13));
        let right = bounds((12, 10), (13, 13));
        let yards = [
            (
                first,
                Yard {
                    min: left.min,
                    max: left.max,
                },
            ),
            (
                second,
                Yard {
                    min: right.min,
                    max: right.max,
                },
            ),
        ];
        for y in 10..=13 {
            world.add_grid_obstacle((10, y));
        }
        let area = bounds((10, 10), (13, 13));
        let plan = build_zone_plan(
            TaskModeZoneType::Stockpile,
            area.min,
            &area,
            &world,
            &yards,
            &[],
        )
        .unwrap();
        let ZonePlan::Stockpile { cells, skipped } = plan else {
            panic!("stockpile plan");
        };
        assert_eq!(cells.len(), 12);
        assert_eq!(skipped.len(), 4);
        assert!(
            cells
                .iter()
                .all(|(grid, owner)| *owner == if grid.0 == 11 { first } else { second })
        );
        let outside = bounds((9, 10), (13, 13));
        assert_eq!(
            build_zone_plan(
                TaskModeZoneType::Stockpile,
                outside.min,
                &outside,
                &world,
                &yards,
                &[]
            ),
            Err(ZoneReject::OutsideYard)
        );
    }

    #[test]
    fn yard_expansion_reports_bounds_and_rejects_unchanged_or_other_yard() {
        let world = WorldMap::default();
        let mut entities = World::new();
        let first = entities.spawn_empty().id();
        let second = entities.spawn_empty().id();
        let old = bounds((10, 10), (29, 29));
        let area = bounds((10, 10), (30, 29));
        let mut yards = vec![(
            first,
            Yard {
                min: old.min,
                max: old.max,
            },
        )];
        let outside = bounds((10, 10), (MAP_WIDTH, 29));
        assert_eq!(
            build_zone_plan(
                TaskModeZoneType::Yard,
                old.min,
                &outside,
                &world,
                &yards,
                &[]
            ),
            Err(ZoneReject::OutsideMap)
        );
        assert_eq!(
            build_zone_plan(TaskModeZoneType::Yard, old.min, &old, &world, &yards, &[]),
            Err(ZoneReject::Unchanged)
        );
        let plan =
            build_zone_plan(TaskModeZoneType::Yard, old.min, &area, &world, &yards, &[]).unwrap();
        let ZonePlan::Yard {
            owner,
            previous,
            bounds,
            added_tiles,
        } = plan
        else {
            panic!("yard plan");
        };
        assert_eq!(owner, first);
        assert_eq!(previous, old);
        assert_eq!(bounds, area);
        assert_eq!(added_tiles, 20);
        yards.push((
            second,
            Yard {
                min: area.min,
                max: area.max,
            },
        ));
        assert_eq!(
            build_zone_plan(TaskModeZoneType::Yard, old.min, &area, &world, &yards, &[]),
            Err(ZoneReject::YardOverlap)
        );
    }
}
