//! Stockpileグループ構築ユーティリティ

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use hw_core::constants::TILE_SIZE;
use hw_world::zones::Yard;

use crate::zone::{Stockpile, StockpilePolicy};
use hw_spatial::StockpileSpatialGrid;

type StockpilesQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<Stockpile>, With<StockpilePolicy>)>;

/// ファミリアのTaskArea内にあるStockpileのグループ
pub struct StockpileGroup {
    pub cells: Vec<Entity>,
    pub owner_yard: Entity,
    pub representative: Entity,
}

/// StockpileGroup の探索用空間インデックス
pub struct StockpileGroupSpatialIndex {
    groups_by_owner: HashMap<Entity, Vec<usize>>,
    owner_yards: HashMap<Entity, Yard>,
    owners_by_cell: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
}

impl Default for StockpileGroupSpatialIndex {
    fn default() -> Self {
        Self {
            groups_by_owner: HashMap::new(),
            owner_yards: HashMap::new(),
            owners_by_cell: HashMap::new(),
            cell_size: TILE_SIZE * 2.0,
        }
    }
}

const TASK_AREA_PERIMETER_SEARCH_RADIUS_TILES: f32 = 10.0;

fn pos_to_cell(pos: Vec2, cell_size: f32) -> (i32, i32) {
    (
        (pos.x / cell_size).floor() as i32,
        (pos.y / cell_size).floor() as i32,
    )
}

pub fn build_stockpile_groups(
    stockpile_grid: &StockpileSpatialGrid,
    active_yards: &[(Entity, Yard)],
    q_stockpiles: &StockpilesQuery,
) -> Vec<StockpileGroup> {
    let mut groups = Vec::new();

    for (yard_entity, yard) in active_yards {
        let stock_entities = stockpile_grid.get_in_area(yard.min, yard.max);

        let mut cells_with_positions = Vec::new();

        for stock_entity in &stock_entities {
            let Ok((entity, transform)) = q_stockpiles.get(*stock_entity) else {
                continue;
            };

            let pos = transform.translation.truncate();
            cells_with_positions.push((entity, pos));
        }

        if cells_with_positions.is_empty() {
            continue;
        }

        cells_with_positions.sort_unstable_by(
            |(left_entity, left_pos), (right_entity, right_pos)| {
                left_pos
                    .x
                    .total_cmp(&right_pos.x)
                    .then_with(|| left_pos.y.total_cmp(&right_pos.y))
                    .then_with(|| {
                        (left_entity.index_u32(), left_entity.generation().to_bits()).cmp(&(
                            right_entity.index_u32(),
                            right_entity.generation().to_bits(),
                        ))
                    })
            },
        );

        let centroid = cells_with_positions
            .iter()
            .map(|(_, pos)| *pos)
            .sum::<Vec2>()
            / cells_with_positions.len() as f32;

        let representative = cells_with_positions
            .iter()
            .min_by(|(left_entity, left_pos), (right_entity, right_pos)| {
                left_pos
                    .distance_squared(centroid)
                    .total_cmp(&right_pos.distance_squared(centroid))
                    .then_with(|| left_pos.x.total_cmp(&right_pos.x))
                    .then_with(|| left_pos.y.total_cmp(&right_pos.y))
                    .then_with(|| {
                        (left_entity.index_u32(), left_entity.generation().to_bits()).cmp(&(
                            right_entity.index_u32(),
                            right_entity.generation().to_bits(),
                        ))
                    })
            })
            .map(|(entity, _)| *entity)
            .expect("non-empty stockpile group has a representative");
        let cells = cells_with_positions
            .into_iter()
            .map(|(entity, _)| entity)
            .collect();

        groups.push(StockpileGroup {
            representative,
            cells,
            owner_yard: *yard_entity,
        });
    }

    groups
}

pub fn build_group_spatial_index(
    groups: &[StockpileGroup],
    yards: &[(Entity, Yard)],
) -> StockpileGroupSpatialIndex {
    let mut groups_by_owner: HashMap<Entity, Vec<usize>> = HashMap::new();
    let mut owner_yards: HashMap<Entity, Yard> = HashMap::new();
    let mut owners_by_cell: HashMap<(i32, i32), Vec<Entity>> = HashMap::new();
    let search_radius = TASK_AREA_PERIMETER_SEARCH_RADIUS_TILES * TILE_SIZE;
    let cell_size = search_radius.max(TILE_SIZE * 2.0);

    for (yard_entity, yard) in yards {
        owner_yards.insert(*yard_entity, yard.clone());
        let expanded_min = yard.min - Vec2::splat(search_radius);
        let expanded_max = yard.max + Vec2::splat(search_radius);
        let min_cell = pos_to_cell(expanded_min, cell_size);
        let max_cell = pos_to_cell(expanded_max, cell_size);
        for cy in min_cell.1..=max_cell.1 {
            for cx in min_cell.0..=max_cell.0 {
                owners_by_cell
                    .entry((cx, cy))
                    .or_default()
                    .push(*yard_entity);
            }
        }
    }

    for (group_idx, group) in groups.iter().enumerate() {
        groups_by_owner
            .entry(group.owner_yard)
            .or_default()
            .push(group_idx);
    }

    StockpileGroupSpatialIndex {
        groups_by_owner,
        owner_yards,
        owners_by_cell,
        cell_size,
    }
}

pub fn find_nearest_group_for_item_indexed<'a>(
    item_pos: Vec2,
    groups: &'a [StockpileGroup],
    index: &StockpileGroupSpatialIndex,
) -> Option<&'a StockpileGroup> {
    let search_radius = TASK_AREA_PERIMETER_SEARCH_RADIUS_TILES * TILE_SIZE;
    let search_radius_sq = search_radius * search_radius;

    let mut candidate_group_indices = HashSet::new();
    let mut owners_containing_item = HashSet::new();
    let mut owner_perimeter_dist_sq = HashMap::new();

    let item_cell = pos_to_cell(item_pos, index.cell_size);
    let mut owner_candidates: Vec<Entity> = index
        .owners_by_cell
        .get(&item_cell)
        .cloned()
        .unwrap_or_default();
    if owner_candidates.is_empty() {
        owner_candidates.extend(index.owner_yards.keys().copied());
    }
    owner_candidates.sort_unstable();
    owner_candidates.dedup();

    for owner in owner_candidates {
        let Some(yard) = index.owner_yards.get(&owner) else {
            continue;
        };
        if yard.contains(item_pos) {
            owners_containing_item.insert(owner);
        }

        let perimeter_dist_sq = distance_sq_to_yard_perimeter(item_pos, yard);
        if perimeter_dist_sq <= search_radius_sq {
            owner_perimeter_dist_sq.insert(owner, perimeter_dist_sq);
            if let Some(owner_groups) = index.groups_by_owner.get(&owner) {
                candidate_group_indices.extend(owner_groups.iter().copied());
            }
        }
    }

    let mut best: Option<(usize, f32)> = None;

    for group_idx in candidate_group_indices {
        let group = &groups[group_idx];
        let Some(&perimeter_dist_sq) = owner_perimeter_dist_sq.get(&group.owner_yard) else {
            continue;
        };

        let in_owner_task_area = owners_containing_item.contains(&group.owner_yard);
        if !in_owner_task_area && !owners_containing_item.is_empty() {
            continue;
        }

        let dist = perimeter_dist_sq;

        match &best {
            None => best = Some((group_idx, dist)),
            Some((best_idx, best_dist)) => {
                if dist < *best_dist
                    || (dist == *best_dist && group.owner_yard < groups[*best_idx].owner_yard)
                {
                    best = Some((group_idx, dist));
                }
            }
        }
    }

    best.map(|(idx, _)| &groups[idx])
}

fn distance_sq_to_yard_perimeter(pos: Vec2, yard: &Yard) -> f32 {
    let inside_x = pos.x >= yard.min.x && pos.x <= yard.max.x;
    let inside_y = pos.y >= yard.min.y && pos.y <= yard.max.y;

    if inside_x && inside_y {
        let dist_to_left = pos.x - yard.min.x;
        let dist_to_right = yard.max.x - pos.x;
        let dist_to_bottom = pos.y - yard.min.y;
        let dist_to_top = yard.max.y - pos.y;
        let min_dist = dist_to_left
            .min(dist_to_right)
            .min(dist_to_bottom)
            .min(dist_to_top);
        min_dist * min_dist
    } else {
        let clamped_x = pos.x.clamp(yard.min.x, yard.max.x);
        let clamped_y = pos.y.clamp(yard.min.y, yard.max.y);
        let dx = pos.x - clamped_x;
        let dy = pos.y - clamped_y;
        dx * dx + dy * dy
    }
}
