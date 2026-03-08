use crate::terrain::TerrainType;
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainBorderKind {
    Edge,
    Corner,
    InnerCorner,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainBorderSpec {
    pub grid: (i32, i32),
    pub terrain: TerrainType,
    pub kind: TerrainBorderKind,
    pub rotation_radians: f32,
}

const EDGE_DIRS: [(i32, i32, f32); 4] = [
    (0, 1, 0.0),
    (1, 0, -PI / 2.0),
    (0, -1, PI),
    (-1, 0, PI / 2.0),
];

const CORNER_DIRS: [(i32, i32, f32, usize, usize); 4] = [
    (1, 1, 0.0, 0, 1),
    (1, -1, -PI / 2.0, 2, 1),
    (-1, -1, PI, 2, 3),
    (-1, 1, PI / 2.0, 0, 3),
];

pub fn generate_terrain_border_specs(
    tiles: &[TerrainType],
    map_width: i32,
    map_height: i32,
) -> Vec<TerrainBorderSpec> {
    let mut specs = Vec::new();

    for y in 0..map_height {
        for x in 0..map_width {
            let Some(current) = terrain_at(tiles, map_width, map_height, x, y) else {
                continue;
            };
            let current_priority = current.priority();
            let mut edge_neighbor_priority = [0u8; 4];

            for (dir_idx, &(dx, dy, angle)) in EDGE_DIRS.iter().enumerate() {
                let nx = x + dx;
                let ny = y + dy;
                let Some(neighbor) = terrain_at(tiles, map_width, map_height, nx, ny) else {
                    continue;
                };
                let neighbor_priority = neighbor.priority();
                edge_neighbor_priority[dir_idx] = neighbor_priority;

                if neighbor_priority > current_priority {
                    specs.push(TerrainBorderSpec {
                        grid: (x, y),
                        terrain: neighbor,
                        kind: TerrainBorderKind::Edge,
                        rotation_radians: angle,
                    });
                }
            }

            for &(dx, dy, angle, edge_a, edge_b) in &CORNER_DIRS {
                let nx = x + dx;
                let ny = y + dy;
                let Some(neighbor) = terrain_at(tiles, map_width, map_height, nx, ny) else {
                    continue;
                };
                let neighbor_priority = neighbor.priority();

                if neighbor_priority > current_priority
                    && edge_neighbor_priority[edge_a] <= current_priority
                    && edge_neighbor_priority[edge_b] <= current_priority
                {
                    specs.push(TerrainBorderSpec {
                        grid: (x, y),
                        terrain: neighbor,
                        kind: TerrainBorderKind::Corner,
                        rotation_radians: angle,
                    });
                }

                if edge_neighbor_priority[edge_a] > current_priority
                    && edge_neighbor_priority[edge_b] > current_priority
                {
                    let dominant_priority =
                        edge_neighbor_priority[edge_a].max(edge_neighbor_priority[edge_b]);
                    let dominant_terrain = if edge_neighbor_priority[edge_a] == dominant_priority {
                        terrain_at(
                            tiles,
                            map_width,
                            map_height,
                            x + EDGE_DIRS[edge_a].0,
                            y + EDGE_DIRS[edge_a].1,
                        )
                    } else {
                        terrain_at(
                            tiles,
                            map_width,
                            map_height,
                            x + EDGE_DIRS[edge_b].0,
                            y + EDGE_DIRS[edge_b].1,
                        )
                    };

                    if let Some(terrain) = dominant_terrain {
                        specs.push(TerrainBorderSpec {
                            grid: (x, y),
                            terrain,
                            kind: TerrainBorderKind::InnerCorner,
                            rotation_radians: angle,
                        });
                    }
                }
            }
        }
    }

    specs
}

fn terrain_at(
    tiles: &[TerrainType],
    map_width: i32,
    map_height: i32,
    x: i32,
    y: i32,
) -> Option<TerrainType> {
    if x < 0 || x >= map_width || y < 0 || y >= map_height {
        return None;
    }

    let idx = (y * map_width + x) as usize;
    tiles.get(idx).copied()
}
