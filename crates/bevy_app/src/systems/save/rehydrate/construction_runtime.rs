use super::*;

/// Rebuilds construction-only runtime state from durable tiles before the
/// paused load frame can resume Spatial or Logic. `WorldMap` remains the
/// durable obstacle authority here: rebuilding a curing footprint must not
/// reserve it a second time.
pub(super) fn rehydrate_construction_runtime(world: &mut World) {
    let floor_tiles: Vec<(Entity, Entity, (i32, i32), FloorTileState)> = {
        let mut query = world.query::<(Entity, &FloorTileBlueprint)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.parent_site, tile.grid_pos, tile.state))
            .collect()
    };
    let wall_tiles: Vec<(Entity, Entity, WallTileState)> = {
        let mut query = world.query::<(Entity, &WallTileBlueprint)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.parent_site, tile.state))
            .collect()
    };

    if !world.contains_resource::<TileSiteIndex>() {
        world.insert_resource(TileSiteIndex::default());
    }
    {
        let mut tile_index = world.resource_mut::<TileSiteIndex>();
        tile_index.rebuild_from_tiles(
            floor_tiles
                .iter()
                .map(|(entity, site, _, _)| (*entity, *site)),
            wall_tiles.iter().map(|(entity, site, _)| (*entity, *site)),
        );
        // Stable index order makes any later index-backed mutation deterministic
        // after a dynamically deserialized world replacement.
        for entities in tile_index.floor_tiles_by_site.values_mut() {
            entities.sort_unstable_by_key(|entity| entity.to_bits());
        }
        for entities in tile_index.wall_tiles_by_site.values_mut() {
            entities.sort_unstable_by_key(|entity| entity.to_bits());
        }
    }

    let mut floor_tiles_by_site: FloorTilesBySite = HashMap::new();
    for (entity, site, grid, state) in floor_tiles {
        floor_tiles_by_site
            .entry(site)
            .or_default()
            .push((entity, grid, state));
    }
    let mut wall_tiles_by_site: HashMap<Entity, Vec<WallTileState>> = HashMap::new();
    for (_, site, state) in wall_tiles {
        wall_tiles_by_site.entry(site).or_default().push(state);
    }

    {
        let mut sites = world.query::<(Entity, &mut FloorConstructionSite)>();
        for (site_entity, mut site) in sites.iter_mut(world) {
            let tiles = floor_tiles_by_site
                .get(&site_entity)
                .map(Vec::as_slice)
                .unwrap_or_default();
            site.tiles_reinforced = tiles
                .iter()
                .filter(|(_, _, state)| floor_tile_is_reinforced(*state))
                .count() as u32;
            site.tiles_poured = tiles
                .iter()
                .filter(|(_, _, state)| *state == FloorTileState::Complete)
                .count() as u32;

            let index_matches_total =
                site.tiles_total > 0 && tiles.len() == site.tiles_total as usize;
            if index_matches_total
                && site.phase == FloorConstructionPhase::Reinforcing
                && site.tiles_reinforced == site.tiles_total
            {
                site.phase = FloorConstructionPhase::Pouring;
            }
            if index_matches_total
                && site.phase == FloorConstructionPhase::Pouring
                && site.tiles_poured == site.tiles_total
            {
                site.phase = FloorConstructionPhase::Curing;
            }
        }
    }
    {
        let mut sites = world.query::<(Entity, &mut WallConstructionSite)>();
        for (site_entity, mut site) in sites.iter_mut(world) {
            let tiles = wall_tiles_by_site
                .get(&site_entity)
                .map(Vec::as_slice)
                .unwrap_or_default();
            site.tiles_framed = tiles
                .iter()
                .filter(|state| wall_tile_is_framed(**state))
                .count() as u32;
            site.tiles_coated = tiles
                .iter()
                .filter(|state| **state == WallTileState::Complete)
                .count() as u32;

            if site.tiles_total > 0
                && tiles.len() == site.tiles_total as usize
                && site.phase == WallConstructionPhase::Framing
                && site.tiles_framed == site.tiles_total
            {
                site.phase = WallConstructionPhase::Coating;
            }
        }
    }

    let curing_footprints: CuringFootprints = {
        let mut sites = world.query::<(Entity, &FloorConstructionSite)>();
        sites
            .iter(world)
            .filter(|(_, site)| site.phase == FloorConstructionPhase::Curing)
            .filter_map(|(site_entity, site)| {
                let tiles = floor_tiles_by_site.get(&site_entity)?;
                (site.tiles_total > 0 && tiles.len() == site.tiles_total as usize).then(|| {
                    (
                        site_entity,
                        tiles
                            .iter()
                            .map(|(entity, grid, _)| (*entity, *grid))
                            .collect(),
                    )
                })
            })
            .collect()
    };
    let curing_sites: HashSet<Entity> = curing_footprints
        .iter()
        .map(|(site_entity, _)| *site_entity)
        .collect();
    let stale_footprints: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<CuringFootprint>>();
        query
            .iter(world)
            .filter(|entity| !curing_sites.contains(entity))
            .collect()
    };
    for site_entity in stale_footprints {
        world.entity_mut(site_entity).remove::<CuringFootprint>();
    }
    for (site_entity, tiles) in curing_footprints {
        world
            .entity_mut(site_entity)
            .insert(CuringFootprint::from_tile_positions(tiles));
    }
}

fn floor_tile_is_reinforced(state: FloorTileState) -> bool {
    matches!(
        state,
        FloorTileState::ReinforcedComplete
            | FloorTileState::WaitingMud
            | FloorTileState::PouringReady
            | FloorTileState::Pouring { .. }
            | FloorTileState::Complete
    )
}

fn wall_tile_is_framed(state: WallTileState) -> bool {
    matches!(
        state,
        WallTileState::FramedProvisional
            | WallTileState::WaitingMud
            | WallTileState::CoatingReady
            | WallTileState::Coating { .. }
            | WallTileState::Complete
    )
}
