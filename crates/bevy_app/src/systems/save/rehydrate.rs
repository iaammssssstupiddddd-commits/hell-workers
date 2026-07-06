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

use hw_core::constants::{TILE_SIZE, Z_ITEM_PICKUP};
use hw_core::familiar::Familiar;
use hw_core::logistics::ResourceType;
use hw_core::relationships::LoadedIn;
use hw_core::soul::DamnedSoul;
use hw_core::visual::SoulTaskHandles;
use hw_jobs::{Building, BuildingType, Rock, Tree, TreeVariant};
use hw_logistics::zone::Stockpile;
use hw_logistics::{Inventory, ResourceItem};
use hw_visual::blueprint::BuildingBounceEffect;

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
        let mut q = world
            .query_filtered::<(Entity, &Familiar, &Transform), Without<Destination>>();
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
        let mut q =
            world.query_filtered::<(Entity, &TreeVariant), (With<Tree>, Without<Sprite>)>();
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
        let mut q = world
            .query_filtered::<(Entity, &ResourceItem, Option<&LoadedIn>), Without<Sprite>>();
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
