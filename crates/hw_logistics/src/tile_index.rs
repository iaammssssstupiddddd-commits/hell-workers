use bevy::prelude::*;
use std::collections::HashMap;

use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};

#[derive(Resource, Default)]
pub struct TileSiteIndex {
    pub floor_tiles_by_site: HashMap<Entity, Vec<Entity>>,
    pub wall_tiles_by_site: HashMap<Entity, Vec<Entity>>,
}

pub fn sync_floor_tile_site_index_system(
    mut tile_site_index: ResMut<TileSiteIndex>,
    q_added: Query<(Entity, &FloorTileBlueprint), Added<FloorTileBlueprint>>,
) {
    for (tile_entity, tile) in q_added.iter() {
        let list = tile_site_index
            .floor_tiles_by_site
            .entry(tile.parent_site)
            .or_default();
        if !list.contains(&tile_entity) {
            list.push(tile_entity);
        }
    }
}

fn sync_removed_tiles(
    map: &mut HashMap<Entity, Vec<Entity>>,
    removed_tiles: impl Iterator<Item = Entity>,
) {
    let mut cleanup_sites: Vec<Entity> = Vec::new();
    for tile_entity in removed_tiles {
        for (site_entity, tile_entities) in map.iter_mut() {
            tile_entities.retain(|entity| *entity != tile_entity);
            if tile_entities.is_empty() {
                cleanup_sites.push(*site_entity);
            }
        }
    }

    for site_entity in cleanup_sites {
        map.remove(&site_entity);
    }
}

pub fn sync_removed_floor_tile_site_index_system(
    mut tile_site_index: ResMut<TileSiteIndex>,
    mut q_removed: RemovedComponents<FloorTileBlueprint>,
) {
    sync_removed_tiles(&mut tile_site_index.floor_tiles_by_site, q_removed.read());
}

pub fn sync_wall_tile_site_index_system(
    mut tile_site_index: ResMut<TileSiteIndex>,
    q_added: Query<(Entity, &WallTileBlueprint), Added<WallTileBlueprint>>,
) {
    for (tile_entity, tile) in q_added.iter() {
        let list = tile_site_index
            .wall_tiles_by_site
            .entry(tile.parent_site)
            .or_default();
        if !list.contains(&tile_entity) {
            list.push(tile_entity);
        }
    }
}

pub fn sync_removed_wall_tile_site_index_system(
    mut tile_site_index: ResMut<TileSiteIndex>,
    mut q_removed: RemovedComponents<WallTileBlueprint>,
) {
    sync_removed_tiles(&mut tile_site_index.wall_tiles_by_site, q_removed.read());
}
