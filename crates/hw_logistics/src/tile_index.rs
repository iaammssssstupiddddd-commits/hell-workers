use bevy::prelude::*;
use std::collections::HashMap;

use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};

#[derive(Resource, Default)]
pub struct TileSiteIndex {
    pub floor_tiles_by_site: HashMap<Entity, Vec<Entity>>,
    pub wall_tiles_by_site: HashMap<Entity, Vec<Entity>>,
    floor_tile_to_site: HashMap<Entity, Entity>,
    wall_tile_to_site: HashMap<Entity, Entity>,
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
        tile_site_index
            .floor_tile_to_site
            .insert(tile_entity, tile.parent_site);
    }
}

fn remove_tile_from_site(
    map: &mut HashMap<Entity, Vec<Entity>>,
    tile_to_site: &mut HashMap<Entity, Entity>,
    tile_entity: Entity,
) {
    let Some(site_entity) = tile_to_site.remove(&tile_entity) else {
        return;
    };
    if let Some(tile_entities) = map.get_mut(&site_entity) {
        tile_entities.retain(|entity| *entity != tile_entity);
        if tile_entities.is_empty() {
            map.remove(&site_entity);
        }
    }
}

fn sync_removed_tiles(
    map: &mut HashMap<Entity, Vec<Entity>>,
    tile_to_site: &mut HashMap<Entity, Entity>,
    removed_tiles: impl Iterator<Item = Entity>,
) {
    for tile_entity in removed_tiles {
        remove_tile_from_site(map, tile_to_site, tile_entity);
    }
}

pub fn sync_removed_floor_tile_site_index_system(
    mut tile_site_index: ResMut<TileSiteIndex>,
    mut q_removed: RemovedComponents<FloorTileBlueprint>,
) {
    let removed: Vec<_> = q_removed.read().collect();
    let TileSiteIndex {
        floor_tiles_by_site,
        floor_tile_to_site,
        ..
    } = tile_site_index.as_mut();
    sync_removed_tiles(floor_tiles_by_site, floor_tile_to_site, removed.into_iter());
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
        tile_site_index
            .wall_tile_to_site
            .insert(tile_entity, tile.parent_site);
    }
}

pub fn sync_removed_wall_tile_site_index_system(
    mut tile_site_index: ResMut<TileSiteIndex>,
    mut q_removed: RemovedComponents<WallTileBlueprint>,
) {
    let removed: Vec<_> = q_removed.read().collect();
    let TileSiteIndex {
        wall_tiles_by_site,
        wall_tile_to_site,
        ..
    } = tile_site_index.as_mut();
    sync_removed_tiles(wall_tiles_by_site, wall_tile_to_site, removed.into_iter());
}
