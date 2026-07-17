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

type GridPosition = (i32, i32);
type RehydratedFloorTile = (Entity, GridPosition, FloorTileState);
type RehydratedFloorTiles = Vec<RehydratedFloorTile>;
type FloorTilesBySite = HashMap<Entity, RehydratedFloorTiles>;
type CuringFootprintTile = (Entity, GridPosition);
type CuringFootprintSpec = (Entity, Vec<CuringFootprintTile>);
type CuringFootprints = Vec<CuringFootprintSpec>;

mod prerequisites;

pub(super) use prerequisites::{RehydratePrerequisiteError, validate_rehydrate_prerequisites};

mod presentation;

pub(super) use presentation::clear_rehydrate_presentation;

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

mod construction_runtime;

use construction_runtime::rehydrate_construction_runtime;

mod obstacles;

use obstacles::rehydrate_obstacle_runtime;

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

mod construction_shells;

use construction_shells::{BlueprintSpriteHandles, rehydrate_construction_shells};

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
#[path = "rehydrate/tests/mod.rs"]
mod tests;
