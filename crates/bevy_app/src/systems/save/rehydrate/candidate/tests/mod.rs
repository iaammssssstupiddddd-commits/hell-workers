use super::topology::{
    validate_construction_links, validate_energy_links, validate_world_map_candidate,
};
use super::*;
use hw_core::relationships::{CommandedBy, LoadedItems, ManagedBy, ParkedWheelbarrows};
use hw_world::WorldMap;

mod contracts;
mod familiar;
mod logistics;
mod shell;
mod topology;

fn candidate_world() -> World {
    let mut world = World::new();
    world.insert_resource(WorldMap::default());
    world
}
