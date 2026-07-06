mod access;
mod bridges;
mod buildings;
mod doors;
mod obstacles;
mod stockpiles;
mod tiles;

pub use access::{WorldMapRead, WorldMapWrite};

use crate::TerrainType;
use crate::pathfinding::PathWorld;
use bevy::ecs::entity::EntityMapper;
use bevy::prelude::*;
use bevy::reflect::{ReflectDeserialize, ReflectSerialize};
use hw_core::GridPos;
use hw_core::world::DoorState;
use std::collections::{HashMap, HashSet};

/// Save/load: `WorldMap` is saved as a `Resource` so that terrain, buildings,
/// doors, and stockpile placement survive a save/load cycle (see
/// `docs/save_load.md`). Entity references embedded in the `HashMap`/`Vec`
/// fields use the `#[component(map_entities = ...)]` override (calling
/// [`map_world_map_entities`]) instead of per-field `#[entities]` attributes,
/// because `HashMap<(i32, i32), Entity>` keys don't implement `MapEntities`
/// (the derive-generated per-field mapping requires the *whole field type*
/// to implement `MapEntities`, which tuples of primitives do not).
///
/// ⚠️ serde derive + `#[reflect(Serialize, Deserialize)]` は必須:
/// `HashMap<(i32, i32), _>` / `HashSet<(i32, i32)>` のタプルキーは `reflect_hash` を
/// 持たないため、reflect 構造経由の RON デシリアライズは `DynamicMap::insert_boxed`
/// の hash 要求で panic する（bevy_reflect 0.19 の制約）。`ReflectDeserialize` 型
/// データがあると `WorldMap` 全体が serde 経路で具象デシリアライズされ、dynamic
/// 表現を経由しない。Entity remap（`map_world_map_entities`）は apply 時に走るので
/// serde 経路でも有効。
#[derive(Resource, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Resource, Serialize, Deserialize)]
#[component(map_entities = map_world_map_entities)]
pub struct WorldMap {
    pub tiles: Vec<TerrainType>,
    pub tile_entities: Vec<Option<Entity>>,
    pub buildings: HashMap<(i32, i32), Entity>,
    pub doors: HashMap<(i32, i32), Entity>,
    pub door_states: HashMap<(i32, i32), DoorState>,
    pub stockpiles: HashMap<(i32, i32), Entity>,
    pub bridged_tiles: HashSet<(i32, i32)>,
    pub obstacles: Vec<bool>,
}

impl Default for WorldMap {
    fn default() -> Self {
        use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            tiles: vec![TerrainType::Grass; size],
            tile_entities: vec![None; size],
            buildings: HashMap::new(),
            doors: HashMap::new(),
            door_states: HashMap::new(),
            stockpiles: HashMap::new(),
            bridged_tiles: HashSet::new(),
            obstacles: vec![false; size],
        }
    }
}

/// `#[component(map_entities = ...)]` override for [`WorldMap`] (see the
/// doc comment on the struct for why this can't use `#[entities]` fields).
fn map_world_map_entities<M: EntityMapper>(this: &mut WorldMap, mapper: &mut M) {
    for entity in this.tile_entities.iter_mut().flatten() {
        *entity = mapper.get_mapped(*entity);
    }
    for entity in this.buildings.values_mut() {
        *entity = mapper.get_mapped(*entity);
    }
    for entity in this.doors.values_mut() {
        *entity = mapper.get_mapped(*entity);
    }
    for entity in this.stockpiles.values_mut() {
        *entity = mapper.get_mapped(*entity);
    }
    // `door_states`, `bridged_tiles`, `tiles`, `obstacles` carry no Entity references.
}

impl PathWorld for WorldMap {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        WorldMap::pos_to_idx(self, x, y)
    }

    fn idx_to_pos(&self, idx: usize) -> GridPos {
        WorldMap::idx_to_pos(idx)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        WorldMap::is_walkable(self, x, y)
    }

    fn get_door_cost(&self, x: i32, y: i32) -> i32 {
        WorldMap::get_door_cost(self, x, y)
    }
}
