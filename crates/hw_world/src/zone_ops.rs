//! Zone 操作に関する純粋なアルゴリズム helper。
//! `Query` / `Commands` に依存せず、`WorldMap` と domain 型のみを扱う。

use std::collections::HashMap;

use hw_core::area::AreaBounds;

use crate::coords::world_to_grid;
use crate::map::WorldMap;
use crate::zones::{Site, Yard};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StockpileRemovalCell {
    pub grid: (i32, i32),
    pub entity: bevy::prelude::Entity,
    pub yard: bevy::prelude::Entity,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct StockpileRemovalTargets {
    pub direct: Vec<StockpileRemovalCell>,
    pub fragments: Vec<StockpileRemovalCell>,
}

impl StockpileRemovalTargets {
    pub fn cells(&self) -> impl Iterator<Item = &StockpileRemovalCell> {
        self.direct.iter().chain(&self.fragments)
    }
}

fn take_component(
    start: (i32, i32),
    yard: bevy::prelude::Entity,
    remaining: &mut HashMap<(i32, i32), StockpileRemovalCell>,
) -> Vec<StockpileRemovalCell> {
    let mut component = vec![remaining.remove(&start).expect("component start exists")];
    let mut head = 0;
    while head < component.len() {
        let (x, y) = component[head].grid;
        head += 1;
        for grid in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if remaining.get(&grid).is_some_and(|cell| cell.yard == yard) {
                component.extend(remaining.remove(&grid));
            }
        }
    }
    component
}

/// Finds direct removals and detached fragments per original, same-Yard component.
pub fn identify_removal_targets(
    cells: impl IntoIterator<Item = StockpileRemovalCell>,
    area: &AreaBounds,
) -> StockpileRemovalTargets {
    let min = WorldMap::world_to_grid(area.min + bevy::math::Vec2::splat(0.1));
    let max = WorldMap::world_to_grid(area.max - bevy::math::Vec2::splat(0.1));
    let inside = |(x, y): (i32, i32)| x >= min.0 && x <= max.0 && y >= min.1 && y <= max.1;

    let mut remaining = HashMap::new();
    let mut targets = StockpileRemovalTargets::default();
    for cell in cells {
        if inside(cell.grid) {
            targets.direct.push(cell);
        }
        remaining.insert(cell.grid, cell);
    }

    for &cell in &targets.direct {
        if !remaining.contains_key(&cell.grid) {
            continue;
        }
        let original = take_component(cell.grid, cell.yard, &mut remaining);
        let mut survivors: HashMap<_, _> = original
            .into_iter()
            .filter(|cell| !inside(cell.grid))
            .map(|cell| (cell.grid, cell))
            .collect();
        let mut keep = Vec::<StockpileRemovalCell>::new();
        while let Some(&start) = survivors.keys().next() {
            let mut fragment = take_component(start, cell.yard, &mut survivors);
            let rank = |cells: &[StockpileRemovalCell]| {
                (cells.len(), cells.iter().map(|cell| cell.grid).min())
            };
            if rank(&fragment) > rank(&keep) {
                std::mem::swap(&mut keep, &mut fragment);
            }
            targets.fragments.extend(fragment);
        }
    }
    targets.direct.sort_unstable();
    targets.fragments.sort_unstable();
    targets
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// エリアのタイル単位サイズを返す。`(幅, 高さ)`
pub fn area_tile_size(area: &AreaBounds) -> (usize, usize) {
    let min_grid = world_to_grid(area.min + bevy::math::Vec2::splat(0.1));
    let max_grid = world_to_grid(area.max - bevy::math::Vec2::splat(0.1));
    let width = max_grid.0.saturating_sub(min_grid.0).saturating_add(1) as usize;
    let height = max_grid.1.saturating_sub(min_grid.1).saturating_add(1) as usize;
    (width, height)
}

/// `area` と `site` が矩形重複しているか判定する。
pub fn rectangles_overlap_site(area: &AreaBounds, site: &Site) -> bool {
    area.min.x < site.max.x
        && area.max.x > site.min.x
        && area.min.y < site.max.y
        && area.max.y > site.min.y
}

/// `area` と `yard` が矩形重複しているか判定する。
pub fn rectangles_overlap(area: &AreaBounds, yard: &Yard) -> bool {
    area.min.x <= yard.max.x
        && area.max.x >= yard.min.x
        && area.min.y <= yard.max.y
        && area.max.y >= yard.min.y
}

/// `yard` を `drag_area` を含むように拡張した `AreaBounds` を返す。
pub fn expand_yard_area(yard: &Yard, drag_area: &AreaBounds) -> AreaBounds {
    let min = bevy::math::Vec2::new(
        yard.min.x.min(drag_area.min.x),
        yard.min.y.min(drag_area.min.y),
    );
    let max = bevy::math::Vec2::new(
        yard.max.x.max(drag_area.max.x),
        yard.max.y.max(drag_area.max.y),
    );
    AreaBounds { min, max }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use hw_core::constants::TILE_SIZE;

    fn cell_area(x: i32, y: i32) -> AreaBounds {
        let center = WorldMap::grid_to_world(x, y);
        let half = Vec2::splat(TILE_SIZE * 0.5);
        AreaBounds::new(center - half, center + half)
    }

    #[test]
    fn removing_an_end_keeps_every_untouched_component_test() {
        let mut world = World::new();
        let yard = world.spawn_empty().id();
        let other_yard = world.spawn_empty().id();
        let mut cell = |x, y, yard| StockpileRemovalCell {
            grid: (x, y),
            entity: world.spawn_empty().id(),
            yard,
        };
        let end = cell(10, 10, yard);
        let cells = [
            end,
            cell(11, 10, yard),
            cell(12, 10, yard),
            cell(13, 10, yard),
            cell(14, 10, other_yard),
            cell(15, 10, other_yard),
            cell(21, 20, yard),
            cell(22, 20, yard),
        ];
        let result = identify_removal_targets(cells, &cell_area(10, 10));
        assert_eq!(result.direct, [end]);
        assert!(result.fragments.is_empty());
    }

    #[test]
    fn equally_sized_fragments_keep_the_larger_minimum_grid_test() {
        let mut world = World::new();
        let yard = world.spawn_empty().id();
        let cells: Vec<_> = (10..=14)
            .rev()
            .map(|x| StockpileRemovalCell {
                grid: (x, 10),
                entity: world.spawn_empty().id(),
                yard,
            })
            .collect();
        let result = identify_removal_targets(cells.iter().copied(), &cell_area(12, 10));
        assert_eq!(result.direct, [cells[2]]);
        assert_eq!(result.fragments, [cells[4], cells[3]]);
    }
}
