//! ロード後の「再水和」（rehydration）。
//!
//! セーブは simulation 状態（`schema.rs` の allow-list）のみを復元するため、
//! ロード直後のエンティティは spawn 時に付与される実行時コンポーネント
//! （ビジュアル・AI 状態・移動・随伴エンティティ）を欠いた「裸」の状態になる。
//! このモジュールが `load_world_system` の最後に呼ばれ、各カテゴリの shell を再付与する。
//!
//! shell の実体は各 spawn モジュール側の `attach_*_shell` 関数（spawn とロードで共用）:
//! - Soul: `entities::damned_soul::spawn::attach_soul_shell`
//! - Familiar: `entities::familiar::attach_familiar_shell`
//! - Building: `systems::jobs::attach_building_shell`
//!
//! Blueprint と floor / wall construction の visual mirror と Sprite は save schema
//! から意図的に除外されるため、durable state からここで明示的に再構築する。
//! これにより、`GameSystemSet::Logic` が停止中のロードでも Visual phase が完全な
//! construction state を観測できる。

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::entities::damned_soul::spawn::attach_soul_shell;
use crate::entities::damned_soul::{Destination, SoulIdentity};
use crate::entities::familiar::attach_familiar_shell;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::attach_building_shell;
use crate::systems::jobs::floor_construction::CuringFootprint;
use crate::world::map::WorldMap;

use hw_core::constants::{TILE_SIZE, Z_ITEM_PICKUP};
use hw_core::familiar::Familiar;
use hw_core::jobs::WorkType;
use hw_core::logistics::ResourceType;
use hw_core::relationships::LoadedIn;
use hw_core::soul::DamnedSoul;
use hw_core::visual::SoulTaskHandles;
use hw_core::visual_mirror::construction::{
    BlueprintVisualState, FloorSiteVisualState, FloorTileVisualMirror, WallSiteVisualState,
    WallTileVisualMirror,
};
use hw_core::world::DoorState;
use hw_jobs::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use hw_jobs::visual_sync::{
    blueprint_visual_state, floor_site_visual_state, floor_tile_visual_mirror,
    wall_site_visual_state, wall_tile_visual_mirror,
};
use hw_jobs::{
    Blueprint, Building, BuildingType, Designation, Door, ObstaclePosition, ObstacleSourceKind,
    Rock, Tree, TreeVariant,
};
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::zone::Stockpile;
use hw_logistics::{Inventory, ResourceItem};
use hw_ui::selection::building_size;
use hw_visual::SoulProxyOwnerCache;
use hw_visual::blueprint::{BlueprintVisual, BuildingBounceEffect};
use hw_visual::visual3d::{
    Building3dVisual, FamiliarProxy3d, SoulMaskProxy3d, SoulProxy3d, SoulShadowProxy3d,
};
use hw_world::seed_obstacle_position_index;
use std::collections::{HashMap, HashSet};
use std::fmt;

type GridPosition = (i32, i32);
type RehydratedFloorTile = (Entity, GridPosition, FloorTileState);
type RehydratedFloorTiles = Vec<RehydratedFloorTile>;
type FloorTilesBySite = HashMap<Entity, RehydratedFloorTiles>;
type CuringFootprintTile = (Entity, GridPosition);
type CuringFootprintSpec = (Entity, Vec<CuringFootprintTile>);
type CuringFootprints = Vec<CuringFootprintSpec>;

/// The subset of loaded assets needed to recreate a regular building Blueprint.
///
/// Construction state itself is asset-independent; keeping this narrow lets the
/// rehydration path be tested without constructing the full `GameAssets` catalog.
#[derive(Default)]
struct BlueprintSpriteHandles {
    wall_isolated: Handle<Image>,
    door_closed: Handle<Image>,
    mud_floor: Handle<Image>,
    tank_empty: Handle<Image>,
    mud_mixer: Handle<Image>,
    rest_area: Handle<Image>,
    bridge: Handle<Image>,
    sand_pile: Handle<Image>,
    bone_pile: Handle<Image>,
    wheelbarrow_parking: Handle<Image>,
}

impl From<&GameAssets> for BlueprintSpriteHandles {
    fn from(assets: &GameAssets) -> Self {
        Self {
            wall_isolated: assets.wall_isolated.clone(),
            door_closed: assets.door_closed.clone(),
            mud_floor: assets.mud_floor.clone(),
            tank_empty: assets.tank_empty.clone(),
            mud_mixer: assets.mud_mixer.clone(),
            rest_area: assets.rest_area.clone(),
            bridge: assets.bridge.clone(),
            sand_pile: assets.sand_pile.clone(),
            bone_pile: assets.bone_pile.clone(),
            wheelbarrow_parking: assets.wheelbarrow_parking.clone(),
        }
    }
}

impl BlueprintSpriteHandles {
    fn sprite(&self, kind: BuildingType) -> Sprite {
        let image = match kind {
            BuildingType::Wall => self.wall_isolated.clone(),
            BuildingType::Door => self.door_closed.clone(),
            BuildingType::Floor => self.mud_floor.clone(),
            BuildingType::Tank => self.tank_empty.clone(),
            BuildingType::MudMixer => self.mud_mixer.clone(),
            BuildingType::RestArea => self.rest_area.clone(),
            BuildingType::Bridge => self.bridge.clone(),
            BuildingType::SandPile => self.sand_pile.clone(),
            BuildingType::BonePile | BuildingType::SoulSpa | BuildingType::OutdoorLamp => {
                self.bone_pile.clone()
            }
            BuildingType::WheelbarrowParking => self.wheelbarrow_parking.clone(),
        };

        Sprite {
            image,
            color: Color::srgba(1.0, 1.0, 1.0, 0.5),
            custom_size: Some(building_size(kind)),
            ..default()
        }
    }
}

/// Resources required to rebuild runtime shells after a persistent world replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RehydratePrerequisiteError {
    missing_resources: Vec<&'static str>,
    invalid_conditions: Vec<&'static str>,
}

impl fmt::Display for RehydratePrerequisiteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot rehydrate: missing resource(s): {}; invalid condition(s): {}",
            self.missing_resources.join(", "),
            self.invalid_conditions.join(", ")
        )
    }
}

/// Validates resources consumed by rehydration before the live persisted world is despawned.
pub(super) fn validate_rehydrate_prerequisites(
    world: &World,
) -> Result<(), RehydratePrerequisiteError> {
    let mut missing_resources = Vec::new();

    macro_rules! require_resource {
        ($type:ty) => {
            if !world.contains_resource::<$type>() {
                missing_resources.push(std::any::type_name::<$type>());
            }
        };
    }

    require_resource!(GameAssets);
    require_resource!(Building3dHandles);
    require_resource!(SoulTaskHandles);
    require_resource!(WorldMap);

    let mut invalid_conditions = Vec::new();
    if let Some(game_assets) = world.get_resource::<GameAssets>()
        && game_assets.trees.is_empty()
    {
        invalid_conditions.push("GameAssets.trees must not be empty");
    }

    if missing_resources.is_empty() && invalid_conditions.is_empty() {
        Ok(())
    } else {
        Err(RehydratePrerequisiteError {
            missing_resources,
            invalid_conditions,
        })
    }
}

/// Removes only presentation entities created by this module's shell helpers.
/// This is deliberately narrower than M4's general load-reset registry.
pub(super) fn clear_rehydrate_presentation(world: &mut World) {
    let presentation_entities: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, Or<(
            With<SoulProxy3d>,
            With<SoulMaskProxy3d>,
            With<SoulShadowProxy3d>,
            With<FamiliarProxy3d>,
            With<Building3dVisual>,
            With<crate::entities::familiar::FamiliarRangeIndicator>,
        )>>();
        query.iter(world).collect()
    };
    for entity in presentation_entities {
        world.despawn(entity);
    }
    if world.contains_resource::<SoulProxyOwnerCache>() {
        world.insert_resource(SoulProxyOwnerCache::default());
    }
}

/// ロード直後に呼び、裸のエンティティへ shell を再付与する。
pub(super) fn rehydrate_after_load(world: &mut World) -> Result<(), RehydratePrerequisiteError> {
    validate_rehydrate_prerequisites(world)?;
    drop_orphaned_inventory_items(world);

    world.resource_scope::<GameAssets, _>(|world, game_assets| {
        world.resource_scope::<Building3dHandles, _>(|world, handles_3d| {
            world.resource_scope::<SoulTaskHandles, _>(|world, soul_handles| {
                rehydrate_shells(world, &game_assets, &handles_3d, &soul_handles);
            });
        });
    });

    world.flush();
    rehydrate_construction_runtime(world);
    rehydrate_obstacle_runtime(world);

    Ok(())
}

/// Rebuilds construction-only runtime state from durable tiles before the
/// paused load frame can resume Spatial or Logic. `WorldMap` remains the
/// durable obstacle authority here: rebuilding a curing footprint must not
/// reserve it a second time.
fn rehydrate_construction_runtime(world: &mut World) {
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

/// Restores runtime obstacle provenance and derives the raw bitmap from durable
/// load state. `WorldMap.obstacles` is a cache, not a save-format authority.
fn rehydrate_obstacle_runtime(world: &mut World) {
    let (natural_owners, natural_blockers) = restore_natural_obstacle_sources(world);
    let (curing_tiles, mut blockers) = restore_curing_floor_protection(world);
    blockers.extend(natural_blockers);
    despawn_incomplete_move_designations(world);
    discard_non_durable_obstacle_markers(world, &natural_owners, &curing_tiles);

    let map_sources = collect_world_map_obstacle_sources(world);
    blockers.extend(map_sources.blockers.iter().copied());
    apply_world_map_obstacle_sources(world, &map_sources, &blockers);
    spawn_building_obstacle_mirrors(world, &map_sources.building_mirrors);

    // Marker/source restoration precedes seeding so the first runtime removal
    // has an old position and provenance even when no Added event is visible.
    seed_obstacle_position_index(world);
}

fn restore_natural_obstacle_sources(world: &mut World) -> (HashSet<Entity>, HashSet<(i32, i32)>) {
    let natural_markers: Vec<(Entity, Option<(i32, i32)>)> = {
        let mut query = world
            .query_filtered::<(Entity, Option<&ObstaclePosition>), Or<(With<Tree>, With<Rock>)>>();
        query
            .iter(world)
            .map(|(entity, position)| (entity, position.map(|position| (position.0, position.1))))
            .collect()
    };

    let mut natural_owners = HashSet::new();
    let mut blockers = HashSet::new();
    for (entity, position) in natural_markers {
        if let Some(grid) = position {
            world
                .entity_mut(entity)
                .insert(ObstacleSourceKind::NaturalTerrainClearing);
            natural_owners.insert(entity);
            blockers.insert(grid);
        } else {
            warn!(
                "REHYDRATE: natural obstacle {entity:?} has no ObstaclePosition; skipping blocker recovery"
            );
        }
    }
    (natural_owners, blockers)
}

fn restore_curing_floor_protection(world: &mut World) -> (HashSet<Entity>, HashSet<(i32, i32)>) {
    let curing_sites: HashSet<Entity> = {
        let mut query = world.query::<(Entity, &FloorConstructionSite)>();
        query
            .iter(world)
            .filter(|(_, site)| site.phase == FloorConstructionPhase::Curing)
            .map(|(entity, _)| entity)
            .collect()
    };
    let floor_tiles: Vec<(Entity, Entity, (i32, i32))> = {
        let mut query = world.query::<(Entity, &FloorTileBlueprint)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.parent_site, tile.grid_pos))
            .collect()
    };

    let mut curing_tiles = HashSet::new();
    let mut blockers = HashSet::new();
    for (tile_entity, site_entity, grid) in floor_tiles {
        if curing_sites.contains(&site_entity) {
            world.entity_mut(tile_entity).insert((
                ObstaclePosition(grid.0, grid.1),
                ObstacleSourceKind::ConstructionProtection,
            ));
            curing_tiles.insert(tile_entity);
            blockers.insert(grid);
        } else {
            world
                .entity_mut(tile_entity)
                .remove::<(ObstaclePosition, ObstacleSourceKind)>();
        }
    }
    (curing_tiles, blockers)
}

fn despawn_incomplete_move_designations(world: &mut World) {
    let move_designations: Vec<Entity> = {
        let mut query = world.query::<(Entity, &Designation)>();
        query
            .iter(world)
            .filter(|(_, designation)| designation.work_type == WorkType::Move)
            .map(|(entity, _)| entity)
            .collect()
    };

    for entity in move_designations {
        world.despawn(entity);
    }
}

fn discard_non_durable_obstacle_markers(
    world: &mut World,
    natural_owners: &HashSet<Entity>,
    curing_tiles: &HashSet<Entity>,
) {
    let source_markers: Vec<(Entity, ObstacleSourceKind)> = {
        let mut query = world.query::<(Entity, &ObstacleSourceKind)>();
        query
            .iter(world)
            .map(|(entity, source)| (entity, *source))
            .collect()
    };

    for (entity, source) in source_markers {
        let keep = match source {
            ObstacleSourceKind::NaturalTerrainClearing => natural_owners.contains(&entity),
            ObstacleSourceKind::ConstructionProtection => curing_tiles.contains(&entity),
            ObstacleSourceKind::BuildingFootprint | ObstacleSourceKind::PlacementReservation => {
                false
            }
        };
        if keep {
            continue;
        }

        match source {
            ObstacleSourceKind::BuildingFootprint | ObstacleSourceKind::PlacementReservation => {
                world.despawn(entity);
            }
            ObstacleSourceKind::NaturalTerrainClearing
            | ObstacleSourceKind::ConstructionProtection => {
                world
                    .entity_mut(entity)
                    .remove::<(ObstaclePosition, ObstacleSourceKind)>();
            }
        }
    }

    // No other persisted entity is an obstacle source. Dropping stale marker
    // data prevents a pre-M4 save from reviving an incomplete reservation.
    let unclassified_markers: Vec<Entity> = {
        let mut query =
            world.query_filtered::<Entity, (With<ObstaclePosition>, Without<ObstacleSourceKind>)>();
        query.iter(world).collect()
    };
    for entity in unclassified_markers {
        world.entity_mut(entity).remove::<ObstaclePosition>();
    }
}

struct WorldMapObstacleSources {
    blockers: HashSet<(i32, i32)>,
    building_mirrors: Vec<(Entity, (i32, i32))>,
    stale_building_entries: Vec<(i32, i32)>,
    doors: HashMap<(i32, i32), (Entity, DoorState)>,
    bridged_tiles: HashSet<(i32, i32)>,
}

fn collect_world_map_obstacle_sources(world: &World) -> WorldMapObstacleSources {
    let map_entries: Vec<((i32, i32), Entity)> = world
        .resource::<WorldMap>()
        .building_entries()
        .map(|(&grid, &entity)| (grid, entity))
        .collect();
    let saved_door_states = world.resource::<WorldMap>().door_states.clone();

    let mut sources = WorldMapObstacleSources {
        blockers: HashSet::new(),
        building_mirrors: Vec::new(),
        stale_building_entries: Vec::new(),
        doors: HashMap::new(),
        bridged_tiles: HashSet::new(),
    };

    for (grid, owner) in map_entries {
        if let Some(building) = world.get::<Building>(owner) {
            if building.kind == BuildingType::Bridge {
                sources.bridged_tiles.insert(grid);
            }
            if building.kind.blocks_movement() {
                sources.blockers.insert(grid);
                sources.building_mirrors.push((owner, grid));
            }
            if building.kind == BuildingType::Door {
                let state = saved_door_states
                    .get(&grid)
                    .copied()
                    .or_else(|| world.get::<Door>(owner).map(|door| door.state))
                    .unwrap_or(DoorState::Closed);
                sources.doors.insert(grid, (owner, state));
            }
            continue;
        }

        if let Some(blueprint) = world.get::<Blueprint>(owner) {
            if blueprint.kind != BuildingType::Bridge {
                sources.blockers.insert(grid);
            }
            continue;
        }

        if world.get::<WallConstructionSite>(owner).is_some() {
            sources.blockers.insert(grid);
            continue;
        }

        warn!("REHYDRATE: dropping stale WorldMap building entry at {grid:?} for {owner:?}");
        sources.stale_building_entries.push(grid);
    }
    sources
}

fn apply_world_map_obstacle_sources(
    world: &mut World,
    sources: &WorldMapObstacleSources,
    blockers: &HashSet<(i32, i32)>,
) {
    for &(owner, state) in sources.doors.values() {
        if let Some(mut door) = world.get_mut::<Door>(owner) {
            door.state = state;
        }
    }

    let mut world_map = world.resource_mut::<WorldMap>();
    for grid in &sources.stale_building_entries {
        world_map.clear_building(*grid);
    }
    world_map.replace_navigation_caches(blockers, &sources.doors, &sources.bridged_tiles);
}

fn spawn_building_obstacle_mirrors(world: &mut World, mirrors: &[(Entity, (i32, i32))]) {
    for &(owner, (x, y)) in mirrors {
        world.spawn((
            ChildOf(owner),
            ObstaclePosition(x, y),
            ObstacleSourceKind::BuildingFootprint,
            Name::new("Building Obstacle"),
        ));
    }
}

/// Phase A ではロード後の全 Soul が `AssignedTask::None` になるため、
/// インベントリに残ったアイテムは誰にも消費されない孤児になる。
/// Soul の足元へドロップして通常の物流ループに戻す。
fn drop_orphaned_inventory_items(world: &mut World) {
    let mut drops: Vec<(Entity, Entity, Vec3)> = Vec::new();
    let mut q_souls = world.query_filtered::<(Entity, &Inventory, &Transform), With<DamnedSoul>>();
    for (soul, inventory, transform) in q_souls.iter(world) {
        if let Some(item) = inventory.0 {
            drops.push((soul, item, transform.translation));
        }
    }

    let drop_count = drops.len();
    for (soul, item, soul_pos) in drops {
        if let Some(mut inventory) = world.get_mut::<Inventory>(soul) {
            inventory.0 = None;
        }
        if let Ok(mut item_mut) = world.get_entity_mut(item) {
            if let Some(mut transform) = item_mut.get_mut::<Transform>() {
                transform.translation = Vec3::new(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP);
            }
        } else {
            warn!("REHYDRATE: inventory item {item:?} of soul {soul:?} no longer exists");
        }
    }
    if drop_count > 0 {
        info!("REHYDRATE: dropped {drop_count} orphaned inventory item(s)");
    }
}

fn rehydrate_shells(
    world: &mut World,
    game_assets: &GameAssets,
    handles_3d: &Building3dHandles,
    soul_handles: &SoulTaskHandles,
) {
    // ---- 収集フェーズ（&mut World クエリ） ----
    let rehydrated_souls = rehydrate_soul_shells(world, handles_3d);
    let blueprint_sprite_handles = BlueprintSpriteHandles::from(game_assets);
    rehydrate_construction_shells(world, &blueprint_sprite_handles);

    let mut familiars: Vec<(Entity, String, f32, Vec3)> = Vec::new();
    {
        let mut q = world.query_filtered::<(Entity, &Familiar, &Transform), Without<Destination>>();
        for (entity, familiar, transform) in q.iter(world) {
            familiars.push((
                entity,
                familiar.name.clone(),
                familiar.command_radius,
                transform.translation,
            ));
        }
    }

    let mut trees: Vec<(Entity, usize)> = Vec::new();
    {
        let mut q = world.query_filtered::<(Entity, &TreeVariant), (With<Tree>, Without<Sprite>)>();
        for (entity, variant) in q.iter(world) {
            trees.push((entity, variant.0));
        }
    }

    let rocks: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, (With<Rock>, Without<Sprite>)>();
        q.iter(world).collect()
    };

    let mut items: Vec<(Entity, ResourceType, bool)> = Vec::new();
    {
        let mut q =
            world.query_filtered::<(Entity, &ResourceItem, Option<&LoadedIn>), Without<Sprite>>();
        for (entity, item, loaded_in) in q.iter(world) {
            items.push((entity, item.0, loaded_in.is_some()));
        }
    }

    let mut buildings: Vec<(Entity, BuildingType, bool, Vec2)> = Vec::new();
    {
        let mut q = world
            .query_filtered::<(Entity, &Building, &Transform), Without<BuildingBounceEffect>>();
        for (entity, building, transform) in q.iter(world) {
            buildings.push((
                entity,
                building.kind,
                building.is_provisional,
                transform.translation.truncate(),
            ));
        }
    }

    let stockpiles: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, (With<Stockpile>, Without<Sprite>)>();
        q.iter(world).collect()
    };

    info!(
        "REHYDRATE: souls={} familiars={} trees={} rocks={} items={} buildings={} stockpiles={}",
        rehydrated_souls,
        familiars.len(),
        trees.len(),
        rocks.len(),
        items.len(),
        buildings.len(),
        stockpiles.len(),
    );

    // ---- 適用フェーズ（Commands 経由、rehydrate_after_load 側で flush） ----
    let mut commands = world.commands();

    for (entity, name, command_radius, translation) in familiars {
        // root rotation / scale は旧visual animationの残骸であり、論理座標の
        // consumer は translation だけを読む。ロード直後に正規化して
        // Spatial / proxy の余分な Changed 連鎖を持ち越さない。
        commands.entity(entity).insert(Transform {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        });
        attach_familiar_shell(
            &mut commands,
            entity,
            &name,
            command_radius,
            translation.truncate(),
            game_assets,
            handles_3d,
        );
    }

    for (entity, variant) in trees {
        let image = game_assets.trees[variant % game_assets.trees.len()].clone();
        commands.entity(entity).insert(Sprite {
            image,
            custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
            ..default()
        });
    }

    for entity in rocks {
        commands.entity(entity).insert(Sprite {
            image: game_assets.rock.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 1.2)),
            ..default()
        });
    }

    for (entity, resource_type, is_loaded) in items {
        commands
            .entity(entity)
            .insert(item_sprite(resource_type, game_assets, soul_handles));
        // 猫車積載中のアイテムは地面に描画しない（積載ビジュアルは haul 系システムが担う）
        if is_loaded {
            commands.entity(entity).insert(Visibility::Hidden);
        }
    }

    for (entity, kind, is_provisional, pos2d) in buildings {
        attach_building_shell(
            &mut commands,
            entity,
            kind,
            is_provisional,
            pos2d,
            game_assets,
            handles_3d,
        );
    }

    for entity in stockpiles {
        // zone_placement/placement.rs の Stockpile spawn と同じ見た目
        commands.entity(entity).insert((
            Sprite {
                color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Name::new("Stockpile"),
        ));
    }
}

/// Restores the non-persistent visual shell for construction roots.
///
/// Mirrors are built directly from their durable source rather than waiting for
/// `GameSystemSet::Logic`: a load can occur while virtual time is paused, but
/// the visual systems must still render the saved construction state.
fn rehydrate_construction_shells(world: &mut World, sprite_handles: &BlueprintSpriteHandles) {
    let blueprints: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &Blueprint,
            Option<&BlueprintVisualState>,
            Option<&Sprite>,
            Option<&BlueprintVisual>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(
                |(entity, blueprint, existing_visual_state, sprite, visual, name)| {
                    let visual_state = existing_visual_state
                        .is_none()
                        .then(|| blueprint_visual_state(blueprint));
                    let sprite = sprite
                        .is_none()
                        .then(|| sprite_handles.sprite(blueprint.kind));
                    let visual = visual.is_none().then(|| {
                        let state = visual_state
                            .as_ref()
                            .or(existing_visual_state)
                            .expect("a BlueprintVisual requires a visual state");
                        BlueprintVisual::from_visual_state(state)
                    });
                    let name = name
                        .is_none()
                        .then(|| Name::new(format!("Blueprint ({:?})", blueprint.kind)));
                    (visual_state.is_some()
                        || sprite.is_some()
                        || visual.is_some()
                        || name.is_some())
                    .then_some((entity, visual_state, sprite, visual, name))
                },
            )
            .collect()
    };

    let floor_sites: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &FloorConstructionSite,
            Option<&FloorSiteVisualState>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, site, visual_state, name)| {
                let visual_state = visual_state
                    .is_none()
                    .then(|| floor_site_visual_state(site));
                let name = name.is_none().then(|| Name::new("FloorConstructionSite"));
                (visual_state.is_some() || name.is_some()).then_some((entity, visual_state, name))
            })
            .collect()
    };

    let floor_tiles: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &FloorTileBlueprint,
            Option<&FloorTileVisualMirror>,
            Option<&Sprite>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, tile, visual_state, sprite, name)| {
                let visual_state = visual_state
                    .is_none()
                    .then(|| floor_tile_visual_mirror(tile));
                let sprite = sprite.is_none().then(|| Sprite {
                    color: Color::srgba(0.50, 0.50, 0.80, 0.20),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                });
                let name = name.is_none().then(|| {
                    Name::new(format!(
                        "FloorTile({},{})",
                        tile.grid_pos.0, tile.grid_pos.1
                    ))
                });
                (visual_state.is_some() || sprite.is_some() || name.is_some()).then_some((
                    entity,
                    visual_state,
                    sprite,
                    name,
                ))
            })
            .collect()
    };

    let wall_sites: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &WallConstructionSite,
            Option<&WallSiteVisualState>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, site, visual_state, name)| {
                let visual_state = visual_state.is_none().then(|| wall_site_visual_state(site));
                let name = name.is_none().then(|| Name::new("WallConstructionSite"));
                (visual_state.is_some() || name.is_some()).then_some((entity, visual_state, name))
            })
            .collect()
    };

    let wall_tiles: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &WallTileBlueprint,
            Option<&WallTileVisualMirror>,
            Option<&Sprite>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, tile, visual_state, sprite, name)| {
                let visual_state = visual_state
                    .is_none()
                    .then(|| wall_tile_visual_mirror(tile));
                let sprite = sprite.is_none().then(|| Sprite {
                    color: Color::srgba(0.80, 0.55, 0.30, 0.25),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                });
                let name = name.is_none().then(|| {
                    Name::new(format!("WallTile({},{})", tile.grid_pos.0, tile.grid_pos.1))
                });
                (visual_state.is_some() || sprite.is_some() || name.is_some()).then_some((
                    entity,
                    visual_state,
                    sprite,
                    name,
                ))
            })
            .collect()
    };

    let mut commands = world.commands();
    for (entity, visual_state, sprite, visual, name) in blueprints {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(sprite) = sprite {
            commands.entity(entity).insert(sprite);
        }
        if let Some(visual) = visual {
            commands.entity(entity).insert(visual);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, name) in floor_sites {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, sprite, name) in floor_tiles {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(sprite) = sprite {
            commands.entity(entity).insert(sprite);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, name) in wall_sites {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, sprite, name) in wall_tiles {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(sprite) = sprite {
            commands.entity(entity).insert(sprite);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
}

/// Rehydrates Soul-owned shell state and returns the number of Souls that
/// needed reconstruction. `Destination` is inserted by every shell, making
/// the second call on the same world a no-op for both the owner and its 3D
/// presentation roots.
fn rehydrate_soul_shells(world: &mut World, handles_3d: &Building3dHandles) -> usize {
    let mut souls: Vec<(Entity, Option<SoulIdentity>, String, Vec3)> = Vec::new();
    {
        let mut query = world.query_filtered::<(Entity, Option<&SoulIdentity>, &Transform), (
            With<DamnedSoul>,
            Without<Destination>,
        )>();
        for (entity, identity, transform) in query.iter(world) {
            let translation = transform.translation;
            match identity {
                Some(identity) => souls.push((entity, None, identity.name.clone(), translation)),
                None => {
                    // 旧形式セーブ（SoulIdentity 未保存）へのフォールバック
                    let identity = SoulIdentity::random();
                    let name = identity.name.clone();
                    souls.push((entity, Some(identity), name, translation));
                }
            }
        }
    }

    let count = souls.len();
    let mut commands = world.commands();
    for (entity, new_identity, name, translation) in souls {
        if let Some(identity) = new_identity {
            commands.entity(entity).insert(identity);
        }
        commands.entity(entity).insert(Transform {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        });
        attach_soul_shell(
            &mut commands,
            entity,
            &name,
            translation.truncate(),
            handles_3d,
        );
    }
    count
}

/// 地面アイテムのスプライト。各 spawn 箇所（`terrain_resources.rs` / soul_ai の
/// gather / collect_bone / refine / sand_collect / facilities.rs）と同じ画像・サイズ。
fn item_sprite(
    resource_type: ResourceType,
    game_assets: &GameAssets,
    soul_handles: &SoulTaskHandles,
) -> Sprite {
    let (image, scale) = match resource_type {
        ResourceType::Wood => (soul_handles.wood.clone(), 0.5),
        ResourceType::Rock => (soul_handles.rock.clone(), 0.5),
        ResourceType::Bone => (soul_handles.icon_bone_small.clone(), 0.5),
        ResourceType::Sand => (soul_handles.icon_sand_small.clone(), 0.5),
        ResourceType::StasisMud => (soul_handles.icon_stasis_mud_small.clone(), 0.5),
        ResourceType::BucketEmpty => (soul_handles.bucket_empty.clone(), 0.5),
        ResourceType::Water | ResourceType::BucketWater => (soul_handles.bucket_water.clone(), 0.5),
        ResourceType::Wheelbarrow => (game_assets.wheelbarrow_empty.clone(), 0.6),
    };
    Sprite {
        image,
        custom_size: Some(Vec2::splat(TILE_SIZE * scale)),
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BlueprintSpriteHandles, clear_rehydrate_presentation, rehydrate_construction_runtime,
        rehydrate_construction_shells, rehydrate_obstacle_runtime, rehydrate_soul_shells,
        validate_rehydrate_prerequisites,
    };
    use crate::entities::damned_soul::{Gender, SoulIdentity};
    use crate::plugins::startup::Building3dHandles;
    use crate::systems::jobs::floor_construction::CuringFootprint;
    use crate::world::map::WorldMap;
    use bevy::prelude::*;
    use bevy::time::Virtual;
    use hw_core::area::TaskArea;
    use hw_core::constants::TILE_SIZE;
    use hw_core::jobs::WorkType;
    use hw_core::logistics::ResourceType;
    use hw_core::soul::DamnedSoul;
    use hw_core::system_sets::GameSystemSet;
    use hw_core::visual_mirror::construction::{
        BlueprintVisualState, FloorConstructionPhaseMirror, FloorSiteVisualState,
        FloorTileStateMirror, FloorTileVisualMirror, WallSiteVisualState, WallTileStateMirror,
        WallTileVisualMirror,
    };
    use hw_core::world::DoorState;
    use hw_energy::SoulSpaSite;
    use hw_jobs::construction::{
        FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
        WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
    };
    use hw_jobs::{
        Blueprint, Building, BuildingType, Designation, Door, ObstaclePosition, ObstacleSourceKind,
        Rock, Tree, TreeVariant,
    };
    use hw_logistics::tile_index::TileSiteIndex;
    use hw_visual::MaterialIconHandles;
    use hw_visual::blueprint::{
        BlueprintProgressBars, BlueprintState, BlueprintVisual, DeliveryPopup, MaterialCounter,
        MaterialIcon, material_delivery_vfx_system, spawn_material_display_system,
        spawn_progress_bar_system, update_blueprint_visual_system, update_progress_bar_fill_system,
    };
    use hw_visual::floor_construction::{
        FloorCuringProgressBar, manage_floor_curing_progress_bars_system,
        update_floor_curing_progress_bars_system,
    };
    use hw_visual::wall_construction::{
        WallConstructionProgressBar, manage_wall_progress_bars_system,
        update_wall_progress_bars_system,
    };
    use hw_world::TerrainType;

    fn empty_building_3d_handles() -> Building3dHandles {
        Building3dHandles {
            wall_mesh: Handle::default(),
            wall_material: Handle::default(),
            wall_provisional_material: Handle::default(),
            wall_orientation_aid_mesh: Handle::default(),
            wall_orientation_aid_material: Handle::default(),
            floor_mesh: Handle::default(),
            floor_material: Handle::default(),
            door_mesh: Handle::default(),
            door_material: Handle::default(),
            equipment_1x1_mesh: Handle::default(),
            equipment_2x2_mesh: Handle::default(),
            equipment_material: Handle::default(),
            soul_scene: Handle::default(),
            familiar_mesh: Handle::default(),
            familiar_material: Handle::default(),
            render_layers: bevy::camera::visibility::RenderLayers::default(),
        }
    }

    fn empty_material_icon_handles() -> MaterialIconHandles {
        MaterialIconHandles {
            wood_small: Handle::default(),
            rock_small: Handle::default(),
            sand_small: Handle::default(),
            bone_small: Handle::default(),
            stasis_mud_small: Handle::default(),
            water_small: Handle::default(),
            font_ui: Handle::default(),
        }
    }

    #[derive(Resource, Default)]
    struct LogicRunCount(u32);

    fn count_logic_run(mut count: ResMut<LogicRunCount>) {
        count.0 += 1;
    }

    fn component_count<T: Component>(world: &mut World) -> usize {
        let mut query = world.query::<&T>();
        query.iter(world).count()
    }

    #[test]
    fn prerequisites_are_reported_before_rehydrate_mutates_the_world() {
        let mut world = World::new();
        let durable_entity = world.spawn(DamnedSoul::default()).id();

        assert_eq!(
            validate_rehydrate_prerequisites(&world)
                .unwrap_err()
                .missing_resources,
            vec![
                std::any::type_name::<crate::assets::GameAssets>(),
                std::any::type_name::<crate::plugins::startup::Building3dHandles>(),
                std::any::type_name::<hw_core::visual::SoulTaskHandles>(),
                std::any::type_name::<WorldMap>(),
            ],
        );
        assert!(world.get_entity(durable_entity).is_ok());
    }

    #[test]
    fn presentation_cleanup_removes_only_rehydrate_owned_shells() {
        let mut world = World::new();
        world.init_resource::<hw_visual::SoulProxyOwnerCache>();

        let soul_proxy = world
            .spawn(hw_visual::visual3d::SoulProxy3d {
                owner: Entity::PLACEHOLDER,
                billboard: false,
            })
            .id();
        let mask_proxy = world
            .spawn(hw_visual::visual3d::SoulMaskProxy3d {
                owner: Entity::PLACEHOLDER,
            })
            .id();
        let shadow_proxy = world
            .spawn(hw_visual::visual3d::SoulShadowProxy3d {
                owner: Entity::PLACEHOLDER,
            })
            .id();
        let familiar_proxy = world
            .spawn(hw_visual::visual3d::FamiliarProxy3d {
                owner: Entity::PLACEHOLDER,
            })
            .id();
        let building_visual = world
            .spawn(hw_visual::visual3d::Building3dVisual {
                owner: Entity::PLACEHOLDER,
            })
            .id();
        let range_indicator = world
            .spawn(crate::entities::familiar::FamiliarRangeIndicator(
                Entity::PLACEHOLDER,
            ))
            .id();
        let durable_entity = world.spawn(Tree).id();

        {
            let mut cache = world.resource_mut::<hw_visual::SoulProxyOwnerCache>();
            cache.soul_proxy.insert(Entity::PLACEHOLDER, soul_proxy);
        }

        clear_rehydrate_presentation(&mut world);

        for entity in [
            soul_proxy,
            mask_proxy,
            shadow_proxy,
            familiar_proxy,
            building_visual,
            range_indicator,
        ] {
            assert!(world.get_entity(entity).is_err());
        }
        assert!(world.get_entity(durable_entity).is_ok());
        assert!(
            world
                .resource::<hw_visual::SoulProxyOwnerCache>()
                .soul_proxy
                .is_empty()
        );
    }

    #[test]
    fn soul_shell_rehydrate_is_idempotent() {
        let mut world = World::new();
        let soul = world
            .spawn((
                DamnedSoul::default(),
                SoulIdentity {
                    name: "test soul".to_string(),
                    gender: Gender::Male,
                },
                Transform::from_xyz(2.0, 3.0, 0.0),
            ))
            .id();
        let handles = empty_building_3d_handles();

        assert_eq!(rehydrate_soul_shells(&mut world, &handles), 1);
        world.flush();
        assert!(
            world
                .get::<crate::entities::damned_soul::Destination>(soul)
                .is_some()
        );
        assert_eq!(
            world
                .query::<&hw_visual::visual3d::SoulProxy3d>()
                .iter(&world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query::<&hw_visual::visual3d::SoulMaskProxy3d>()
                .iter(&world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query::<&hw_visual::visual3d::SoulShadowProxy3d>()
                .iter(&world)
                .count(),
            1
        );

        assert_eq!(rehydrate_soul_shells(&mut world, &handles), 0);
        world.flush();
        assert_eq!(
            world
                .query::<&hw_visual::visual3d::SoulProxy3d>()
                .iter(&world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query::<&hw_visual::visual3d::SoulMaskProxy3d>()
                .iter(&world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query::<&hw_visual::visual3d::SoulShadowProxy3d>()
                .iter(&world)
                .count(),
            1
        );
    }

    #[test]
    fn construction_shell_rehydrate_restores_saved_state_while_logic_is_paused() {
        let mut world = World::new();

        let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(3, 4), (4, 4)]);
        blueprint.progress = 0.25;
        blueprint.delivered_materials.insert(ResourceType::Wood, 1);
        let blueprint_entity = world
            .spawn((blueprint, Transform::from_xyz(3.0, 4.0, 0.0)))
            .id();

        let mut floor_site = floor_site(FloorConstructionPhase::Curing);
        floor_site.tiles_total = 3;
        floor_site.curing_remaining_secs = 42.0;
        let floor_site_entity = world.spawn(floor_site).id();
        let mut floor_tile = FloorTileBlueprint::new(floor_site_entity, (5, 6));
        floor_tile.state = FloorTileState::Pouring { progress: 73 };
        floor_tile.bones_delivered = 2;
        let floor_tile_entity = world.spawn(floor_tile).id();

        let mut wall_site = WallConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
            Vec2::ZERO,
            4,
        );
        wall_site.phase = WallConstructionPhase::Coating;
        wall_site.tiles_framed = 4;
        wall_site.tiles_coated = 2;
        let wall_site_entity = world.spawn(wall_site).id();
        let mut wall_tile = WallTileBlueprint::new(wall_site_entity, (7, 8));
        wall_tile.state = WallTileState::Coating { progress: 61 };
        let wall_tile_entity = world.spawn(wall_tile).id();

        rehydrate_construction_shells(&mut world, &BlueprintSpriteHandles::default());
        world.flush();

        let blueprint_visual = world
            .get::<BlueprintVisualState>(blueprint_entity)
            .expect("Blueprint visual state should be restored before the next Logic run");
        assert_eq!(blueprint_visual.progress, 0.25);
        assert!(
            blueprint_visual
                .material_counts
                .contains(&(ResourceType::Wood, 1, 2))
        );
        assert_eq!(
            world
                .get::<BlueprintVisual>(blueprint_entity)
                .and_then(|visual| visual.last_delivered.get(&ResourceType::Wood)),
            Some(&1)
        );
        assert_eq!(
            world
                .get::<Sprite>(blueprint_entity)
                .and_then(|sprite| sprite.custom_size),
            Some(Vec2::splat(TILE_SIZE * 2.0))
        );
        assert_eq!(
            world.get::<Name>(blueprint_entity).map(Name::as_str),
            Some("Blueprint (Tank)")
        );

        let floor_site_visual = world
            .get::<FloorSiteVisualState>(floor_site_entity)
            .expect("floor site visual state should be restored");
        assert_eq!(
            floor_site_visual.phase,
            FloorConstructionPhaseMirror::Curing
        );
        assert_eq!(floor_site_visual.curing_remaining_secs, 42.0);
        assert_eq!(floor_site_visual.tiles_total, 3);
        let floor_tile_visual = world
            .get::<FloorTileVisualMirror>(floor_tile_entity)
            .expect("floor tile visual mirror should be restored");
        assert_eq!(floor_tile_visual.bones_delivered, 2);
        assert_eq!(
            floor_tile_visual.state,
            FloorTileStateMirror::Pouring { progress: 73 }
        );
        assert!(world.get::<Sprite>(floor_tile_entity).is_some());
        assert_eq!(
            world.get::<Name>(floor_site_entity).map(Name::as_str),
            Some("FloorConstructionSite")
        );
        assert_eq!(
            world.get::<Name>(floor_tile_entity).map(Name::as_str),
            Some("FloorTile(5,6)")
        );

        let wall_site_visual = world
            .get::<WallSiteVisualState>(wall_site_entity)
            .expect("wall site visual state should be restored");
        assert!(!wall_site_visual.phase_is_framing);
        assert_eq!(wall_site_visual.tiles_total, 4);
        assert_eq!(wall_site_visual.tiles_framed, 4);
        assert_eq!(wall_site_visual.tiles_coated, 2);
        let wall_tile_visual = world
            .get::<WallTileVisualMirror>(wall_tile_entity)
            .expect("wall tile visual mirror should be restored");
        assert_eq!(
            wall_tile_visual.state,
            WallTileStateMirror::Coating { progress: 61 }
        );
        assert!(world.get::<Sprite>(wall_tile_entity).is_some());
        assert_eq!(
            world.get::<Name>(wall_site_entity).map(Name::as_str),
            Some("WallConstructionSite")
        );
        assert_eq!(
            world.get::<Name>(wall_tile_entity).map(Name::as_str),
            Some("WallTile(7,8)")
        );

        let restored_sprite_count = world.query::<&Sprite>().iter(&world).count();
        rehydrate_construction_shells(&mut world, &BlueprintSpriteHandles::default());
        world.flush();
        assert_eq!(
            world.query::<&Sprite>().iter(&world).count(),
            restored_sprite_count
        );
    }

    #[test]
    fn paused_visual_phase_rebuilds_construction_without_delivery_replay() {
        let mut app = App::new();
        app.init_resource::<Time<Virtual>>();
        app.init_resource::<LogicRunCount>();
        app.insert_resource(empty_material_icon_handles());
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.configure_sets(
            Update,
            (
                GameSystemSet::Logic.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                GameSystemSet::Visual,
            )
                .chain(),
        );
        app.add_systems(Update, count_logic_run.in_set(GameSystemSet::Logic));
        app.add_systems(
            Update,
            (
                update_blueprint_visual_system,
                spawn_progress_bar_system,
                update_progress_bar_fill_system,
                spawn_material_display_system,
                material_delivery_vfx_system,
                manage_floor_curing_progress_bars_system,
                update_floor_curing_progress_bars_system,
                manage_wall_progress_bars_system,
                update_wall_progress_bars_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(3, 4), (4, 4)]);
        blueprint.progress = 0.25;
        blueprint.delivered_materials.insert(ResourceType::Wood, 1);
        let blueprint_entity = app
            .world_mut()
            .spawn((blueprint, Transform::from_xyz(3.0, 4.0, 0.0)))
            .id();

        let mut floor_site = floor_site(FloorConstructionPhase::Curing);
        floor_site.curing_remaining_secs = 42.0;
        let floor_site_entity = app
            .world_mut()
            .spawn((floor_site, Transform::from_xyz(5.0, 6.0, 0.0)))
            .id();

        let mut wall_site = WallConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
            Vec2::ZERO,
            3,
        );
        wall_site.tiles_framed = 1;
        let wall_site_entity = app
            .world_mut()
            .spawn((wall_site, Transform::from_xyz(7.0, 8.0, 0.0)))
            .id();

        rehydrate_construction_shells(app.world_mut(), &BlueprintSpriteHandles::default());
        app.world_mut().flush();
        app.update();
        app.update();

        assert_eq!(app.world().resource::<LogicRunCount>().0, 0);
        let visual = app
            .world()
            .get::<BlueprintVisual>(blueprint_entity)
            .expect("load rehydration should attach BlueprintVisual before Visual runs");
        assert_eq!(visual.state, BlueprintState::Building);
        assert_eq!(
            visual.last_delivered.get(&ResourceType::Wood),
            Some(&1),
            "saved deliveries must not be treated as new deliveries"
        );
        assert!(
            app.world()
                .get::<BlueprintProgressBars>(blueprint_entity)
                .is_some(),
            "the paused Visual phase should create the progress bar"
        );
        assert_eq!(component_count::<MaterialIcon>(app.world_mut()), 1);
        assert_eq!(component_count::<MaterialCounter>(app.world_mut()), 1);
        assert_eq!(component_count::<DeliveryPopup>(app.world_mut()), 0);
        assert!(
            app.world()
                .get::<FloorSiteVisualState>(floor_site_entity)
                .is_some(),
            "the paused Visual phase must receive a rehydrated floor site mirror"
        );
        assert_eq!(
            component_count::<FloorCuringProgressBar>(app.world_mut()),
            2
        );
        assert!(
            app.world()
                .get::<WallSiteVisualState>(wall_site_entity)
                .is_some(),
            "the paused Visual phase must receive a rehydrated wall site mirror"
        );
        assert_eq!(
            component_count::<WallConstructionProgressBar>(app.world_mut()),
            2
        );
    }

    fn floor_site(phase: FloorConstructionPhase) -> FloorConstructionSite {
        let mut site = FloorConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
            Vec2::ZERO,
            1,
        );
        site.phase = phase;
        site
    }

    #[test]
    fn construction_runtime_rehydrate_rebuilds_indexes_counters_and_curing_cache() {
        let mut world = World::new();
        world.insert_resource(TileSiteIndex::default());
        world.insert_resource(WorldMap::default());
        let original_obstacle_version = world.resource::<WorldMap>().obstacle_version;
        let original_obstacle_count = world
            .resource::<WorldMap>()
            .obstacles
            .iter()
            .filter(|blocked| **blocked)
            .count();

        let mut floor_site = FloorConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
            Vec2::ZERO,
            2,
        );
        floor_site.phase = FloorConstructionPhase::Curing;
        floor_site.tiles_reinforced = 0;
        floor_site.tiles_poured = 0;
        let floor_site_entity = world.spawn(floor_site).id();
        for grid_pos in [(3, 4), (4, 4)] {
            let mut tile = FloorTileBlueprint::new(floor_site_entity, grid_pos);
            tile.state = FloorTileState::Complete;
            world.spawn(tile);
        }

        let mut wall_site = WallConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
            Vec2::ZERO,
            2,
        );
        wall_site.tiles_framed = 0;
        let wall_site_entity = world.spawn(wall_site).id();
        for grid_pos in [(7, 8), (8, 8)] {
            let mut tile = WallTileBlueprint::new(wall_site_entity, grid_pos);
            tile.state = WallTileState::FramedProvisional;
            world.spawn(tile);
        }

        rehydrate_construction_runtime(&mut world);

        let index = world.resource::<TileSiteIndex>();
        assert_eq!(index.floor_tiles_by_site[&floor_site_entity].len(), 2);
        assert_eq!(index.wall_tiles_by_site[&wall_site_entity].len(), 2);
        let floor = world
            .get::<FloorConstructionSite>(floor_site_entity)
            .expect("floor site remains durable during curing");
        assert_eq!(floor.tiles_reinforced, 2);
        assert_eq!(floor.tiles_poured, 2);
        assert_eq!(floor.phase, FloorConstructionPhase::Curing);
        assert!(world.get::<CuringFootprint>(floor_site_entity).is_some());
        let wall = world
            .get::<WallConstructionSite>(wall_site_entity)
            .expect("wall site remains durable during coating");
        assert_eq!(wall.tiles_framed, 2);
        assert_eq!(wall.tiles_coated, 0);
        assert_eq!(wall.phase, WallConstructionPhase::Coating);
        assert_eq!(
            world.resource::<WorldMap>().obstacle_version,
            original_obstacle_version
        );
        assert_eq!(
            world
                .resource::<WorldMap>()
                .obstacles
                .iter()
                .filter(|blocked| **blocked)
                .count(),
            original_obstacle_count,
            "construction cache rehydration must not reserve the durable map again",
        );
    }

    fn building_mirror_count(world: &mut World, owner: Entity, grid: (i32, i32)) -> usize {
        let mut query = world.query::<(&ObstaclePosition, &ObstacleSourceKind, &ChildOf)>();
        query
            .iter(world)
            .filter(|(position, source, parent)| {
                parent.parent() == owner
                    && **source == ObstacleSourceKind::BuildingFootprint
                    && (position.0, position.1) == grid
            })
            .count()
    }

    #[test]
    fn rebuilds_durable_sources_and_stays_idempotent() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());

        let tree = world
            .spawn((Tree, TreeVariant(0), ObstaclePosition(3, 4)))
            .id();
        let rock = world.spawn((Rock, ObstaclePosition(5, 6))).id();

        let tank = world
            .spawn(Building {
                kind: BuildingType::Tank,
                is_provisional: false,
            })
            .id();
        let blueprint = world
            .spawn(Blueprint::new(BuildingType::Tank, vec![(9, 10)]))
            .id();
        let bridge = world
            .spawn(Building {
                kind: BuildingType::Bridge,
                is_provisional: false,
            })
            .id();
        let spa = world
            .spawn((
                SoulSpaSite::default(),
                Building {
                    kind: BuildingType::SoulSpa,
                    is_provisional: false,
                },
            ))
            .id();

        let open_door = world
            .spawn((
                Building {
                    kind: BuildingType::Door,
                    is_provisional: false,
                },
                Door {
                    state: DoorState::Locked,
                },
            ))
            .id();
        let closed_door = world
            .spawn((
                Building {
                    kind: BuildingType::Door,
                    is_provisional: false,
                },
                Door {
                    state: DoorState::Open,
                },
            ))
            .id();
        let locked_door = world
            .spawn((
                Building {
                    kind: BuildingType::Door,
                    is_provisional: false,
                },
                Door {
                    state: DoorState::Open,
                },
            ))
            .id();

        let curing_site = world.spawn(floor_site(FloorConstructionPhase::Curing)).id();
        let curing_tile = world
            .spawn(FloorTileBlueprint::new(curing_site, (18, 19)))
            .id();
        let unfinished_site = world
            .spawn(floor_site(FloorConstructionPhase::Reinforcing))
            .id();
        let unfinished_tile = world
            .spawn((
                FloorTileBlueprint::new(unfinished_site, (20, 21)),
                ObstaclePosition(20, 21),
                ObstacleSourceKind::ConstructionProtection,
            ))
            .id();
        let move_designation = world
            .spawn(Designation {
                work_type: WorkType::Move,
            })
            .id();

        {
            let mut map = world.resource_mut::<WorldMap>();
            map.set_building((7, 8), tank);
            map.set_building((9, 10), blueprint);
            map.set_building((11, 12), bridge);
            map.set_building((13, 14), spa);

            let bridge_idx = map.pos_to_idx(11, 12).unwrap();
            map.set_terrain_at_idx(bridge_idx, TerrainType::River);
            let stale_bridge_idx = map.pos_to_idx(22, 23).unwrap();
            map.set_terrain_at_idx(stale_bridge_idx, TerrainType::River);
            map.bridged_tiles.insert((22, 23));

            for (grid, door, state) in [
                ((14, 15), open_door, DoorState::Open),
                ((15, 16), closed_door, DoorState::Closed),
                ((16, 17), locked_door, DoorState::Locked),
            ] {
                map.set_building(grid, door);
                map.doors.insert(grid, door);
                map.door_states.insert(grid, state);
            }

            map.add_grid_obstacle((11, 12));
            map.add_grid_obstacle((20, 21));
            map.add_grid_obstacle((22, 23));
        }

        rehydrate_obstacle_runtime(&mut world);

        assert_eq!(
            world.get::<ObstacleSourceKind>(tree),
            Some(&ObstacleSourceKind::NaturalTerrainClearing)
        );
        assert_eq!(
            world.get::<ObstacleSourceKind>(rock),
            Some(&ObstacleSourceKind::NaturalTerrainClearing)
        );
        assert_eq!(building_mirror_count(&mut world, tank, (7, 8)), 1);
        assert_eq!(building_mirror_count(&mut world, blueprint, (9, 10)), 0);
        assert_eq!(building_mirror_count(&mut world, bridge, (11, 12)), 0);

        assert_eq!(
            world.get::<ObstacleSourceKind>(curing_tile),
            Some(&ObstacleSourceKind::ConstructionProtection)
        );
        assert!(world.get::<ObstaclePosition>(curing_tile).is_some());
        assert!(world.get::<ObstaclePosition>(unfinished_tile).is_none());
        assert!(world.get_entity(move_designation).is_err());

        {
            let map = world.resource::<WorldMap>();
            assert!(!map.is_walkable(3, 4));
            assert!(!map.is_walkable(5, 6));
            assert!(!map.is_walkable(7, 8));
            assert!(!map.is_walkable(9, 10));
            assert!(map.is_walkable(11, 12));
            assert!(map.bridged_tiles.contains(&(11, 12)));
            assert!(map.is_walkable(13, 14));
            assert!(map.is_walkable(14, 15));
            assert!(map.is_walkable(15, 16));
            assert!(!map.is_walkable(16, 17));
            assert!(!map.is_walkable(18, 19));
            assert!(map.is_walkable(20, 21));
            assert!(!map.is_walkable(22, 23));
            assert!(!map.bridged_tiles.contains(&(22, 23)));

            let open_idx = map.pos_to_idx(14, 15).unwrap();
            let closed_idx = map.pos_to_idx(15, 16).unwrap();
            let locked_idx = map.pos_to_idx(16, 17).unwrap();
            assert!(!map.obstacles[open_idx]);
            assert!(map.obstacles[closed_idx]);
            assert!(map.obstacles[locked_idx]);
        }
        assert_eq!(world.get::<Door>(open_door).unwrap().state, DoorState::Open);
        assert_eq!(
            world.get::<Door>(closed_door).unwrap().state,
            DoorState::Closed
        );
        assert_eq!(
            world.get::<Door>(locked_door).unwrap().state,
            DoorState::Locked
        );

        rehydrate_obstacle_runtime(&mut world);
        assert_eq!(building_mirror_count(&mut world, tank, (7, 8)), 1);
    }

    #[test]
    fn rehydrate_restores_missing_door_cache_and_bumps_topology_once() {
        let mut world = World::new();
        world.insert_resource(WorldMap::default());

        let grid = (24, 25);
        let door = world
            .spawn((
                Building {
                    kind: BuildingType::Door,
                    is_provisional: false,
                },
                Door {
                    state: DoorState::Closed,
                },
            ))
            .id();

        let version_before_rehydrate = {
            let mut map = world.resource_mut::<WorldMap>();
            map.set_building(grid, door);
            map.add_grid_obstacle(grid);
            assert!(!map.is_walkable(grid.0, grid.1));
            map.obstacle_version
        };

        rehydrate_obstacle_runtime(&mut world);

        {
            let map = world.resource::<WorldMap>();
            assert_eq!(map.door_entity(grid.0, grid.1), Some(door));
            assert_eq!(map.door_state(grid.0, grid.1), Some(DoorState::Closed));
            assert!(map.has_raw_obstacle(grid.0, grid.1));
            assert!(map.is_walkable(grid.0, grid.1));
            assert_eq!(map.obstacle_version, version_before_rehydrate + 1);
        }

        rehydrate_obstacle_runtime(&mut world);
        assert_eq!(
            world.resource::<WorldMap>().obstacle_version,
            version_before_rehydrate + 1
        );
    }
}
