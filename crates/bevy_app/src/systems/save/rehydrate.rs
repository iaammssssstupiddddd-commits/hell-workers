//! ロード後の「再水和」（rehydration）。
//!
//! セーブは simulation 状態（`saving.rs` の allow-list）のみを復元するため、
//! ロード直後のエンティティは spawn 時に付与される実行時コンポーネント
//! （ビジュアル・AI 状態・移動・随伴エンティティ）を欠いた「裸」の状態になる。
//! このモジュールが `load_world_system` の最後に呼ばれ、各カテゴリの shell を再付与する。
//!
//! shell の実体は各 spawn モジュール側の `attach_*_shell` 関数（spawn とロードで共用）:
//! - Soul: `entities::damned_soul::spawn::attach_soul_shell`
//! - Familiar: `entities::familiar::attach_familiar_shell`
//! - Building: `systems::jobs::attach_building_shell`
//!
//! Blueprint / 建設サイト / TaskArea / Site / Yard のビジュアルは visual_mirror 系の
//! 差分検知システム（`Without<*VisualState>` / `Changed<T>` クエリ）が自然に再生成する
//! ため、ここでは扱わない。

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::entities::damned_soul::spawn::attach_soul_shell;
use crate::entities::damned_soul::{Destination, SoulIdentity};
use crate::entities::familiar::attach_familiar_shell;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::attach_building_shell;
use crate::world::map::WorldMap;

use hw_core::constants::{TILE_SIZE, Z_ITEM_PICKUP};
use hw_core::familiar::Familiar;
use hw_core::jobs::WorkType;
use hw_core::logistics::ResourceType;
use hw_core::relationships::LoadedIn;
use hw_core::soul::DamnedSoul;
use hw_core::visual::SoulTaskHandles;
use hw_core::world::DoorState;
use hw_jobs::construction::{FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint};
use hw_jobs::{
    Blueprint, Building, BuildingType, Designation, Door, ObstaclePosition, ObstacleSourceKind,
    Rock, Tree, TreeVariant, WallConstructionSite,
};
use hw_logistics::zone::Stockpile;
use hw_logistics::{Inventory, ResourceItem};
use hw_visual::blueprint::BuildingBounceEffect;
use hw_world::seed_obstacle_position_index;
use std::collections::{HashMap, HashSet};

/// ロード直後に呼び、裸のエンティティへ shell を再付与する。
pub fn rehydrate_after_load(world: &mut World) {
    drop_orphaned_inventory_items(world);

    world.resource_scope::<GameAssets, _>(|world, game_assets| {
        world.resource_scope::<Building3dHandles, _>(|world, handles_3d| {
            world.resource_scope::<SoulTaskHandles, _>(|world, soul_handles| {
                rehydrate_shells(world, &game_assets, &handles_3d, &soul_handles);
            });
        });
    });

    world.flush();
    rehydrate_obstacle_runtime(world);
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

    // Soul: shell 欠落は Destination の有無で判定（shell が必ず挿入する）
    let mut souls: Vec<(Entity, Option<SoulIdentity>, String, Vec2)> = Vec::new();
    {
        let mut q = world.query_filtered::<(Entity, Option<&SoulIdentity>, &Transform), (
            With<DamnedSoul>,
            Without<Destination>,
        )>();
        for (entity, identity, transform) in q.iter(world) {
            let pos = transform.translation.truncate();
            match identity {
                Some(identity) => souls.push((entity, None, identity.name.clone(), pos)),
                None => {
                    // 旧形式セーブ（SoulIdentity 未保存）へのフォールバック
                    let identity = SoulIdentity::random();
                    let name = identity.name.clone();
                    souls.push((entity, Some(identity), name, pos));
                }
            }
        }
    }

    let mut familiars: Vec<(Entity, String, f32, Vec2)> = Vec::new();
    {
        let mut q = world.query_filtered::<(Entity, &Familiar, &Transform), Without<Destination>>();
        for (entity, familiar, transform) in q.iter(world) {
            familiars.push((
                entity,
                familiar.name.clone(),
                familiar.command_radius,
                transform.translation.truncate(),
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
        souls.len(),
        familiars.len(),
        trees.len(),
        rocks.len(),
        items.len(),
        buildings.len(),
        stockpiles.len(),
    );

    // ---- 適用フェーズ（Commands 経由、rehydrate_after_load 側で flush） ----
    let mut commands = world.commands();

    for (entity, new_identity, name, pos) in souls {
        if let Some(identity) = new_identity {
            commands.entity(entity).insert(identity);
        }
        attach_soul_shell(&mut commands, entity, &name, pos, handles_3d);
    }

    for (entity, name, command_radius, pos) in familiars {
        attach_familiar_shell(
            &mut commands,
            entity,
            &name,
            command_radius,
            pos,
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
    use super::rehydrate_obstacle_runtime;
    use crate::world::map::WorldMap;
    use bevy::prelude::*;
    use hw_core::area::TaskArea;
    use hw_core::jobs::WorkType;
    use hw_core::world::DoorState;
    use hw_energy::SoulSpaSite;
    use hw_jobs::construction::{
        FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint,
    };
    use hw_jobs::{
        Blueprint, Building, BuildingType, Designation, Door, ObstaclePosition, ObstacleSourceKind,
        Rock, Tree, TreeVariant,
    };
    use hw_world::TerrainType;

    fn floor_site(phase: FloorConstructionPhase) -> FloorConstructionSite {
        let mut site = FloorConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
            Vec2::ZERO,
            1,
        );
        site.phase = phase;
        site
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
