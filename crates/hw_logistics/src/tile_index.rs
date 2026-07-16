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

impl TileSiteIndex {
    /// Rebuilds both reverse indexes from durable construction tiles.
    ///
    /// This is used by load rehydration before normal Spatial/Logic schedules
    /// resume. Keeping the insertion path shared with incremental updates
    /// prevents a paused load frame from depending on `Added<T>` delivery.
    pub fn rebuild_from_tiles(
        &mut self,
        floor_tiles: impl IntoIterator<Item = (Entity, Entity)>,
        wall_tiles: impl IntoIterator<Item = (Entity, Entity)>,
    ) {
        *self = Self::default();
        for (tile_entity, site_entity) in floor_tiles {
            self.insert_floor_tile(tile_entity, site_entity);
        }
        for (tile_entity, site_entity) in wall_tiles {
            self.insert_wall_tile(tile_entity, site_entity);
        }
    }

    fn insert_floor_tile(&mut self, tile_entity: Entity, site_entity: Entity) {
        let list = self.floor_tiles_by_site.entry(site_entity).or_default();
        if !list.contains(&tile_entity) {
            list.push(tile_entity);
        }
        self.floor_tile_to_site.insert(tile_entity, site_entity);
    }

    fn insert_wall_tile(&mut self, tile_entity: Entity, site_entity: Entity) {
        let list = self.wall_tiles_by_site.entry(site_entity).or_default();
        if !list.contains(&tile_entity) {
            list.push(tile_entity);
        }
        self.wall_tile_to_site.insert(tile_entity, site_entity);
    }
}

pub fn sync_floor_tile_site_index_system(
    mut tile_site_index: ResMut<TileSiteIndex>,
    q_added: Query<(Entity, &FloorTileBlueprint), Added<FloorTileBlueprint>>,
) {
    for (tile_entity, tile) in q_added.iter() {
        tile_site_index.insert_floor_tile(tile_entity, tile.parent_site);
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
        tile_site_index.insert_wall_tile(tile_entity, tile.parent_site);
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

#[cfg(test)]
mod tests {
    use super::TileSiteIndex;
    use bevy::prelude::Entity;

    #[test]
    fn rebuild_replaces_stale_forward_and_reverse_entries() {
        let old_site = Entity::from_bits(1);
        let old_floor = Entity::from_bits(2);
        let floor_site = Entity::from_bits(3);
        let wall_site = Entity::from_bits(4);
        let floor_a = Entity::from_bits(5);
        let floor_b = Entity::from_bits(6);
        let wall_a = Entity::from_bits(7);

        let mut index = TileSiteIndex::default();
        index.rebuild_from_tiles([(old_floor, old_site)], []);
        index.rebuild_from_tiles(
            [(floor_a, floor_site), (floor_b, floor_site)],
            [(wall_a, wall_site)],
        );

        assert!(!index.floor_tiles_by_site.contains_key(&old_site));
        assert!(!index.floor_tile_to_site.contains_key(&old_floor));
        assert_eq!(
            index.floor_tiles_by_site[&floor_site],
            vec![floor_a, floor_b]
        );
        assert_eq!(index.wall_tiles_by_site[&wall_site], vec![wall_a]);
        assert_eq!(index.floor_tile_to_site[&floor_b], floor_site);
        assert_eq!(index.wall_tile_to_site[&wall_a], wall_site);
    }
}
