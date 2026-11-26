//! Only opposed faces of an opening connect. A missing Room is not evidence of outdoor air.
use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_infra::ventilation::{AirBoundary, Opening};
use hw_jobs::{Building, BuildingType};
use hw_world::{Room, WorldMap};
use std::collections::{HashMap, VecDeque};

type Grid = (i32, i32);
// Axis, negative/positive face names, reference pressure on the negative face.
const AXES: [(Grid, &str, &str, f64); 2] = [((1, 0), "西", "東", 1.0), ((0, 1), "南", "北", 0.0)];

fn index((x, y): Grid) -> Option<usize> {
    ((0..MAP_WIDTH).contains(&x) && (0..MAP_HEIGHT).contains(&y))
        .then_some((y * MAP_WIDTH + x) as usize)
}

#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Unseen,
    Wall,
    Outside,
    Zone(usize),
}

fn fill(cells: &mut [Cell], seeds: impl IntoIterator<Item = Grid>, label: Cell) {
    let mut queue = VecDeque::new();
    let mut visit = |grid, queue: &mut VecDeque<Grid>| {
        if let Some(i) = index(grid)
            && cells[i] == Cell::Unseen
        {
            cells[i] = label;
            queue.push_back(grid);
        }
    };
    for grid in seeds {
        visit(grid, &mut queue);
    }
    while let Some((x, y)) = queue.pop_front() {
        for grid in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            visit(grid, &mut queue);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Passage {
    pub opening: Opening,
    pub kind: &'static str,
    pub a_side: &'static str,
    pub b_side: &'static str,
}
pub(super) struct Port {
    pub entity: Entity,
    pub grid: Grid,
    pub passage: Option<Passage>,
}

#[derive(Default)]
pub(crate) struct Layout {
    outside: Vec<bool>,
    pub(super) terminals: HashMap<Grid, (usize, Entity)>,
    pub(super) first_junction: usize,
    pub(super) auxiliary: Vec<Vec<Grid>>,
    pub(super) rooms: Vec<Entity>,
    pub(super) floors: Vec<usize>,
    pub(super) ports: Vec<Port>,
}

impl Layout {
    pub(super) fn is_outside(&self, grid: Grid) -> bool {
        index(grid).is_none_or(|i| self.outside[i])
    }
    pub(super) fn build<'a>(
        rooms: impl Iterator<Item = (Entity, &'a Room)>,
        buildings: impl Iterator<Item = (Entity, &'a Building, &'a Transform)>,
    ) -> Self {
        let mut layout = Self::default();
        let mut cells = vec![Cell::Unseen; (MAP_WIDTH * MAP_HEIGHT) as usize];
        let mut sorted: Vec<_> = rooms.collect();
        sorted.sort_unstable_by_key(|(_, room)| room.tiles.iter().min().copied());
        for (entity, room) in sorted {
            let i = layout.rooms.len();
            layout.rooms.push(entity);
            layout.floors.push(room.tile_count);
            for &grid in &room.tiles {
                if let Some(cell) = index(grid) {
                    cells[cell] = Cell::Zone(i);
                }
            }
        }
        let mut ports = Vec::new();
        for (entity, building, pose) in buildings {
            if building.is_provisional {
                continue;
            }
            let grid = WorldMap::world_to_grid(pose.translation.truncate());
            let Some(i) = index(grid) else { continue };
            if matches!(
                building.kind,
                BuildingType::Wall | BuildingType::Door | BuildingType::ScreamingVent
            ) {
                cells[i] = Cell::Wall;
            }
            if building.kind == BuildingType::ScreamingVent {
                ports.push((grid, entity));
            }
        }
        ports.sort_unstable_by_key(|p| p.0);
        let edges = (0..MAP_WIDTH)
            .flat_map(|x| [(x, 0), (x, MAP_HEIGHT - 1)])
            .chain((0..MAP_HEIGHT).flat_map(|y| [(0, y), (MAP_WIDTH - 1, y)]));
        fill(&mut cells, edges, Cell::Outside);
        // Enclosed courtyards and unfinished interiors can transport air but are
        // neither the outdoor reference nor occupied rooms with a design load.
        for i in 0..cells.len() {
            if cells[i] == Cell::Unseen {
                let zone = layout.floors.len();
                layout.floors.push(0);
                fill(
                    &mut cells,
                    [(i as i32 % MAP_WIDTH, i as i32 / MAP_WIDTH)],
                    Cell::Zone(zone),
                );
            }
        }
        layout.outside = cells.iter().map(|&cell| cell == Cell::Outside).collect();
        layout
            .auxiliary
            .resize(layout.floors.len() - layout.rooms.len(), Vec::new());
        for (i, cell) in cells.iter().enumerate() {
            if let Cell::Zone(z) = *cell
                && z >= layout.rooms.len()
            {
                layout.auxiliary[z - layout.rooms.len()]
                    .push((i as i32 % MAP_WIDTH, i as i32 / MAP_WIDTH));
            }
        }
        let face = |grid| index(grid).map_or(Cell::Outside, |i| cells[i]);
        let by_grid: HashMap<_, _> = ports.iter().enumerate().map(|(i, p)| (p.0, i)).collect();
        let mut candidates = vec![0; ports.len()];
        let mut runs = Vec::new();
        for &((dx, dy), negative, positive, pressure) in &AXES {
            for &((x, y), _) in &ports {
                let before = (x - dx, y - dy);
                if by_grid.contains_key(&before) {
                    continue;
                }
                let mut members = Vec::new();
                let mut after = (x, y);
                while let Some(&i) = by_grid.get(&after) {
                    members.push(i);
                    after = (after.0 + dx, after.1 + dy);
                }
                let (left, right) = (face(before), face(after));
                if left != right
                    && matches!(left, Cell::Zone(_) | Cell::Outside)
                    && matches!(right, Cell::Zone(_) | Cell::Outside)
                {
                    for &i in &members {
                        candidates[i] += 1;
                    }
                    runs.push((members, left, right, negative, positive, pressure));
                }
            }
        }
        layout.ports = ports
            .into_iter()
            .map(|(grid, entity)| Port {
                grid,
                entity,
                passage: None,
            })
            .collect();
        layout.first_junction = layout.floors.len();
        for (members, mut left, right, negative, positive, pressure) in runs {
            // Each physical grille has only one axis, including at a junction.
            if members.iter().any(|&i| candidates[i] != 1) {
                continue;
            }
            let length = members.len();
            let last = members[length - 1];
            let first = members[0];
            let terminal = match (left, right) {
                (Cell::Zone(a), Cell::Outside) => Some((a, first, last)),
                (Cell::Outside, Cell::Zone(a)) => Some((a, last, first)),
                _ => None,
            };
            if let Some((a, inside, outside)) = terminal {
                layout
                    .terminals
                    .insert(layout.ports[inside].grid, (a, layout.ports[outside].entity));
            }
            for (offset, i) in members.into_iter().enumerate() {
                let next = if offset + 1 == length {
                    right
                } else {
                    // The space between successive open faces stores no design load.
                    let zone = layout.floors.len();
                    layout.floors.push(0);
                    layout.auxiliary.push(vec![layout.ports[i].grid]);
                    Cell::Zone(zone)
                };
                let (room, endpoint, a_side, b_side) = match (left, next) {
                    (Cell::Zone(a), Cell::Zone(b)) => (a, AirBoundary::Room(b), negative, positive),
                    (Cell::Zone(a), Cell::Outside) => (
                        a,
                        AirBoundary::Outdoor {
                            pressure: -pressure,
                        },
                        negative,
                        positive,
                    ),
                    (Cell::Outside, Cell::Zone(b)) => {
                        (b, AirBoundary::Outdoor { pressure }, positive, negative)
                    }
                    _ => unreachable!("validated air passage endpoints"),
                };
                let port = &mut layout.ports[i];
                let kind = match endpoint {
                    AirBoundary::Outdoor { .. } => "外壁開口",
                    AirBoundary::Room(_) if length > 1 => "壁内通気",
                    AirBoundary::Room(b)
                        if room >= layout.rooms.len() || b >= layout.rooms.len() =>
                    {
                        "囲い区画の通気"
                    }
                    AirBoundary::Room(_) => "室間通気",
                };
                port.passage = Some(Passage {
                    kind,
                    a_side,
                    b_side,
                    opening: Opening {
                        a: room,
                        b: endpoint,
                        conductance: 1.0,
                    },
                });
                left = next;
            }
        }
        layout
    }
}
