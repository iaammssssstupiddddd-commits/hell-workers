//! Connections between completed equipment, room openings, and underground pipes.
use super::layout::{Layout, Passage};
use bevy::prelude::*;
use hw_infra::air_transport::Wall;
use hw_jobs::{Building, BuildingType, Equipment};
use hw_world::{Room, WorldMap};
use std::collections::{HashMap, HashSet, VecDeque};

pub(super) type Grid = (i32, i32);
pub(super) fn neighbors((x, y): Grid) -> [Grid; 4] {
    [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
}

pub(super) struct Device {
    pub entity: Entity,
    pub kind: Equipment,
    pub grid: Grid,
    pub room: Option<usize>,
    pub pipes: Vec<usize>,
    pub terminal: Option<(usize, usize)>,
    pub condenser: bool,
}

#[derive(Resource)]
pub(crate) struct Services {
    pub(super) layout: Layout,
    pub(super) ports: Vec<hw_infra::ventilation::Opening>,
    pub(super) vent_entities: Vec<(Entity, Passage)>,
    pub(super) devices: Vec<Device>,
    pub(super) room_tiles: Vec<Vec<Grid>>,
    pub(super) zones: HashMap<Grid, usize>,
    pub(super) volumes: Vec<usize>,
    pub(super) walls: Vec<Wall>,
    pub(super) emitters: Vec<(usize, Grid)>,
}

impl Services {
    /// Find a supplied river intake and an indoor basin on its pipe network in this district.
    pub(crate) fn supplied_basin(
        &self,
        intake_eligible: impl Fn(Entity, Grid) -> bool,
        basin_eligible: impl Fn(Entity, Grid) -> bool,
    ) -> Option<(Grid, Grid)> {
        self.devices
            .iter()
            .filter(|d| d.kind == Equipment::Intake && intake_eligible(d.entity, d.grid))
            .flat_map(|intake| {
                self.devices
                    .iter()
                    .filter(|d| {
                        d.kind == Equipment::Basin
                            && d.room.is_some()
                            && basin_eligible(d.entity, d.grid)
                            && d.pipes.iter().any(|pipe| intake.pipes.contains(pipe))
                    })
                    .map(move |basin| (intake.grid, basin.grid))
            })
            .min()
    }

    pub(super) fn build<'a>(
        layout: Layout,
        rooms: impl Iterator<Item = (Entity, &'a Room)>,
        buildings: impl Iterator<Item = (Entity, &'a Building, &'a Transform)>,
    ) -> Self {
        let by_entity: HashMap<_, _> = rooms.collect();
        let mut zones = HashMap::new();
        let mut room_tiles = Vec::new();
        let mut walls = Vec::new();
        let mut wall_rooms: HashMap<Grid, Vec<usize>> = HashMap::new();
        for (i, e) in layout.rooms.iter().enumerate() {
            let room = by_entity[e];
            let mut tiles = room.tiles.clone();
            tiles.sort_unstable();
            for &grid in &tiles {
                zones.insert(grid, i);
            }
            room_tiles.push(tiles);
            for &wall in room.wall_tiles.iter().chain(&room.door_tiles) {
                wall_rooms.entry(wall).or_default().push(i);
            }
        }
        for (wall, mut group) in wall_rooms {
            group.sort_unstable();
            group.dedup();
            if group.len() == 1 && neighbors(wall).iter().any(|&p| layout.is_outside(p)) {
                walls.push((group[0], None, 0.025));
            }
            for (i, &a) in group.iter().enumerate() {
                for &b in &group[i + 1..] {
                    walls.push((a, Some(b), 0.025));
                }
            }
        }
        let mut equipment = Vec::new();
        let mut pipes = HashSet::new();
        let mut ducts = HashSet::new();
        let mut outdoors = HashSet::new();
        let mut emitters = Vec::new();
        for (e, b, t) in buildings.filter(|(_, b, _)| !b.is_provisional) {
            let grid = WorldMap::world_to_grid(t.translation.xy());
            match b.kind {
                BuildingType::Equipment(kind) => {
                    match kind {
                        Equipment::Conduit => {
                            pipes.insert(grid);
                        }
                        Equipment::Duct => {
                            ducts.insert(grid);
                        }
                        Equipment::OutdoorUnit if layout.is_outside(grid) => {
                            outdoors.insert(grid);
                        }
                        _ => {}
                    }
                    equipment.push((e, kind, grid));
                }
                BuildingType::MudMixer | BuildingType::SoulSpa => {
                    if let Some(&zone) = zones.get(&grid) {
                        emitters.push((zone, grid));
                    }
                }
                _ => {}
            }
        }
        let mut networks = HashMap::new();
        let mut network = 0;
        while let Some(&start) = pipes.iter().next() {
            pipes.remove(&start);
            let mut queue = VecDeque::from([start]);
            while let Some(grid) = queue.pop_front() {
                networks.insert(grid, network);
                for p in neighbors(grid) {
                    if pipes.remove(&p) {
                        queue.push_back(p);
                    }
                }
            }
            network += 1;
        }
        let vent_entities: Vec<_> = layout
            .ports
            .iter()
            .filter_map(|p| Some((p.entity, p.passage?)))
            .collect();
        let active_ids: HashMap<_, _> = vent_entities
            .iter()
            .enumerate()
            .map(|(i, &(entity, _))| (entity, i))
            .collect();
        equipment.sort_unstable_by_key(|&(_, k, g)| (g, k as u8));
        let devices = equipment
            .into_iter()
            .map(|(entity, kind, grid)| {
                let room = zones.get(&grid).copied();
                let mut pipe_ids = Vec::new();
                let offsets = if kind == Equipment::Intake {
                    &[(-1, 0), (0, 0), (1, 0), (-1, -1), (0, -1), (1, -1)][..]
                } else {
                    &[(0, 0)][..]
                };
                for &(x, y) in offsets {
                    let tile = (grid.0 + x, grid.1 + y);
                    for p in std::iter::once(tile).chain(neighbors(tile)) {
                        if let Some(&id) = networks.get(&p)
                            && !pipe_ids.contains(&id)
                        {
                            pipe_ids.push(id);
                        }
                    }
                }
                let mut terminal = None;
                if let Some(room) = room
                    && matches!(
                        kind,
                        Equipment::SupplyFan
                            | Equipment::ExhaustFan
                            | Equipment::Chimney
                            | Equipment::Hood
                    )
                {
                    let mut queue = VecDeque::from([(grid, 0)]);
                    let mut seen = HashSet::from([grid]);
                    'path: while let Some((g, n)) = queue.pop_front() {
                        for p in neighbors(g) {
                            if let Some(&(a, e)) = layout.terminals.get(&p)
                                && a == room
                            {
                                terminal = Some((active_ids[&e], n));
                                break 'path;
                            }
                            if ducts.contains(&p) && zones.get(&p) == Some(&room) && seen.insert(p)
                            {
                                queue.push_back((p, n + 1));
                            }
                        }
                    }
                }
                let condenser = kind == Equipment::HeatPump
                    && outdoors
                        .iter()
                        .any(|p| (grid.0 - p.0).abs() + (grid.1 - p.1).abs() <= 2);
                Device {
                    entity,
                    kind,
                    grid,
                    room,
                    pipes: pipe_ids,
                    terminal,
                    condenser,
                }
            })
            .collect();
        room_tiles.extend(layout.auxiliary.iter().cloned());
        let volumes = room_tiles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if i < layout.first_junction {
                    t.len()
                } else {
                    0
                }
            })
            .collect();
        Self {
            ports: vent_entities.iter().map(|(_, p)| p.opening).collect(),
            vent_entities,
            layout,
            room_tiles,
            devices,
            zones,
            walls,
            emitters,
            volumes,
        }
    }
}
