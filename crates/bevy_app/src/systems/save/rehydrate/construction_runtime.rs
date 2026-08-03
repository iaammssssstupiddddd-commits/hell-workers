use super::*;
use hw_jobs::{Designation, Priority, TaskSlots};

type FloorTileRecord = (Entity, Entity, (i32, i32), FloorTileState);
type WallTileRecord = (Entity, Entity, WallTileState, Option<Entity>);
type WallSiteTileRecord = (Entity, WallTileState, Option<Entity>);
type WallTilesBySite = HashMap<Entity, Vec<WallSiteTileRecord>>;

/// Compatibility normalization that must finish before presentation shells
/// read construction phases and counters.
pub(super) fn normalize_construction_state(world: &mut World) {
    let floor_tiles = collect_floor_tiles(world);
    let wall_tiles = collect_wall_tiles(world);
    let floor_tiles_by_site = floor_tiles_by_site(floor_tiles);
    let wall_tiles_by_site = wall_tiles_by_site(wall_tiles);

    let mut pouring_sites = HashSet::new();
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
            let normalized_phase = normalized_floor_phase(
                &site,
                tiles.len(),
                site.tiles_reinforced,
                site.tiles_poured,
            );
            site.phase = normalized_phase;
            if normalized_phase == FloorConstructionPhase::Pouring {
                pouring_sites.insert(site_entity);
            }
        }
    }
    normalize_floor_transition_tiles(world, &floor_tiles_by_site, &pouring_sites);

    let mut coating_sites = HashSet::new();
    {
        let mut sites = world.query::<(Entity, &mut WallConstructionSite)>();
        for (site_entity, mut site) in sites.iter_mut(world) {
            let tiles = wall_tiles_by_site
                .get(&site_entity)
                .map(Vec::as_slice)
                .unwrap_or_default();
            site.tiles_framed = tiles
                .iter()
                .filter(|(_, state, _)| wall_tile_is_framed(*state))
                .count() as u32;
            site.tiles_coated = tiles
                .iter()
                .filter(|(_, state, _)| *state == WallTileState::Complete)
                .count() as u32;

            if site.tiles_total > 0
                && tiles.len() == site.tiles_total as usize
                && site.phase == WallConstructionPhase::Framing
                && site.tiles_framed == site.tiles_total
                && tiles.iter().all(|(_, state, spawned_wall)| {
                    wall_tile_is_ready_for_coating(*state, *spawned_wall)
                })
            {
                site.phase = WallConstructionPhase::Coating;
            }
            if site.phase == WallConstructionPhase::Coating {
                coating_sites.insert(site_entity);
            }
        }
    }
    normalize_wall_transition_tiles(world, &wall_tiles_by_site, &coating_sites);
}

fn normalize_floor_transition_tiles(
    world: &mut World,
    tiles_by_site: &FloorTilesBySite,
    pouring_sites: &HashSet<Entity>,
) {
    let tiles: Vec<_> = pouring_sites
        .iter()
        .flat_map(|site| tiles_by_site.get(site).into_iter().flatten())
        .filter_map(|(entity, _, state)| {
            (*state == FloorTileState::ReinforcedComplete).then_some(*entity)
        })
        .collect();
    for tile_entity in tiles {
        let mut entity = world.entity_mut(tile_entity);
        entity
            .get_mut::<FloorTileBlueprint>()
            .expect("collected floor tile must remain present")
            .state = FloorTileState::WaitingMud;
        entity.remove::<(Designation, TaskSlots, Priority)>();
    }
}

fn normalize_wall_transition_tiles(
    world: &mut World,
    tiles_by_site: &WallTilesBySite,
    coating_sites: &HashSet<Entity>,
) {
    let tiles: Vec<_> = coating_sites
        .iter()
        .flat_map(|site| tiles_by_site.get(site).into_iter().flatten())
        .filter_map(|(entity, state, spawned_wall)| {
            (*state == WallTileState::FramedProvisional && spawned_wall.is_some())
                .then_some(*entity)
        })
        .collect();
    for tile_entity in tiles {
        let mut entity = world.entity_mut(tile_entity);
        entity
            .get_mut::<WallTileBlueprint>()
            .expect("collected wall tile must remain present")
            .state = WallTileState::WaitingMud;
        entity.remove::<(Designation, TaskSlots, Priority)>();
    }
}

/// Computes the compatibility phase without mutating a candidate world. The
/// load candidate walkability view uses this same rule so inventory drop cells
/// are validated against the obstacle topology that rehydration will produce.
pub(super) fn normalized_floor_phase(
    site: &FloorConstructionSite,
    tile_count: usize,
    tiles_reinforced: u32,
    tiles_poured: u32,
) -> FloorConstructionPhase {
    let index_matches_total = site.tiles_total > 0 && tile_count == site.tiles_total as usize;
    let mut phase = site.phase;
    if index_matches_total
        && phase == FloorConstructionPhase::Reinforcing
        && tiles_reinforced == site.tiles_total
    {
        phase = FloorConstructionPhase::Pouring;
    }
    if index_matches_total
        && phase == FloorConstructionPhase::Pouring
        && tiles_poured == site.tiles_total
    {
        phase = FloorConstructionPhase::Curing;
    }
    phase
}

/// Rebuilds construction-only runtime indexes from already-normalized durable
/// state. `WorldMap` remains the durable obstacle authority here: rebuilding a
/// curing footprint must not reserve it a second time.
pub(super) fn rebuild_construction_runtime(world: &mut World) {
    let floor_tiles = collect_floor_tiles(world);
    let wall_tiles = collect_wall_tiles(world);

    if !world.contains_resource::<TileSiteIndex>() {
        world.insert_resource(TileSiteIndex::default());
    }
    {
        let mut tile_index = world.resource_mut::<TileSiteIndex>();
        tile_index.rebuild_from_tiles(
            floor_tiles
                .iter()
                .map(|(entity, site, _, _)| (*entity, *site)),
            wall_tiles
                .iter()
                .map(|(entity, site, _, _)| (*entity, *site)),
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

    let floor_tiles_by_site = floor_tiles_by_site(floor_tiles);
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

#[cfg(test)]
pub(super) fn rehydrate_construction_runtime(world: &mut World) {
    normalize_construction_state(world);
    rebuild_construction_runtime(world);
}

fn collect_floor_tiles(world: &mut World) -> Vec<FloorTileRecord> {
    {
        let mut query = world.query::<(Entity, &FloorTileBlueprint)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.parent_site, tile.grid_pos, tile.state))
            .collect()
    }
}

fn collect_wall_tiles(world: &mut World) -> Vec<WallTileRecord> {
    {
        let mut query = world.query::<(Entity, &WallTileBlueprint)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.parent_site, tile.state, tile.spawned_wall))
            .collect()
    }
}

fn floor_tiles_by_site(floor_tiles: Vec<FloorTileRecord>) -> FloorTilesBySite {
    let mut floor_tiles_by_site: FloorTilesBySite = HashMap::new();
    for (entity, site, grid, state) in floor_tiles {
        floor_tiles_by_site
            .entry(site)
            .or_default()
            .push((entity, grid, state));
    }
    floor_tiles_by_site
}

fn wall_tiles_by_site(wall_tiles: Vec<WallTileRecord>) -> WallTilesBySite {
    let mut wall_tiles_by_site = HashMap::new();
    for (entity, site, state, spawned_wall) in wall_tiles {
        wall_tiles_by_site
            .entry(site)
            .or_insert_with(Vec::new)
            .push((entity, state, spawned_wall));
    }
    wall_tiles_by_site
}

pub(super) fn floor_tile_is_reinforced(state: FloorTileState) -> bool {
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

fn wall_tile_is_ready_for_coating(state: WallTileState, spawned_wall: Option<Entity>) -> bool {
    match state {
        WallTileState::FramedProvisional => spawned_wall.is_some(),
        WallTileState::WaitingMud
        | WallTileState::CoatingReady
        | WallTileState::Coating { .. }
        | WallTileState::Complete => true,
        WallTileState::WaitingWood
        | WallTileState::FramingReady
        | WallTileState::Framing { .. } => false,
    }
}
