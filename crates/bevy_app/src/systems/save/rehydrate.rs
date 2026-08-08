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

use hw_core::area::TaskArea;
use hw_core::constants::TILE_SIZE;
use hw_core::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, FamiliarPolicy,
};
use hw_core::jobs::WorkType;
use hw_core::logistics::ResourceType;
use hw_core::relationships::{Commanding, LoadedIn, RestingIn, StoredIn};
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
use hw_logistics::zone::{Stockpile, StockpilePolicy};
use hw_logistics::{BelongsTo, BucketStorage, PendingBelongsToBlueprint, ResourceItem};
use hw_ui::selection::building_size;
use hw_visual::SoulProxyOwnerCache;
use hw_visual::blueprint::{BlueprintVisual, BuildingBounceEffect};
use hw_visual::visual3d::{
    Building3dVisual, FamiliarProxy3d, SoulMaskProxy3d, SoulProxy3d, SoulShadowProxy3d,
};
use hw_world::{Yard, seed_obstacle_position_index};
use std::collections::{HashMap, HashSet};

type GridPosition = (i32, i32);
type RehydratedFloorTile = (Entity, GridPosition, FloorTileState);
type RehydratedFloorTiles = Vec<RehydratedFloorTile>;
type FloorTilesBySite = HashMap<Entity, RehydratedFloorTiles>;
type CuringFootprintTile = (Entity, GridPosition);
type CuringFootprintSpec = (Entity, Vec<CuringFootprintTile>);
type CuringFootprints = Vec<CuringFootprintSpec>;

mod candidate;
mod deconstruction;
mod registry;
mod task_runtime;

mod prerequisites;

#[cfg(test)]
use prerequisites::RehydratePrerequisiteError;
#[cfg(test)]
pub(super) use prerequisites::validate_rehydrate_prerequisites;

mod presentation;

pub(super) use presentation::clear_rehydrate_presentation;

#[cfg(test)]
pub(super) use candidate::{
    validate_durable_topology_candidate, validate_familiar_candidate,
    validate_task_logistics_candidate,
};
#[cfg(test)]
pub(super) use deconstruction::{
    normalize_completed_floor_ownership, rebuild_deconstruction_runtime,
    validate_deconstruction_orders,
};
#[cfg(test)]
pub(super) fn normalize_task_logistics_runtime_for_test(world: &mut World) {
    task_runtime::normalize_task_logistics_runtime(world);
}
pub(super) use registry::ResolvedRehydratePlan;
use registry::{
    RehydratePhase, register_candidate_validator, register_live_prerequisite,
    register_rehydrate_step,
};

/// Logic-domain adapter. Leaf crates expose callbacks without depending on the
/// root registry; `LogicPlugin` owns this type-erased registration boundary.
pub(crate) fn register_logic_rehydrate_pipeline(app: &mut App) {
    register_candidate_validator(
        app,
        "durable.topology",
        candidate::validate_durable_topology_candidate,
    );
    register_candidate_validator(
        app,
        "familiar.roster",
        candidate::validate_familiar_candidate,
    );
    register_candidate_validator(
        app,
        "task-logistics.owners",
        candidate::validate_task_logistics_candidate,
    );
    register_candidate_validator(
        app,
        "deconstruction.orders",
        deconstruction::validate_deconstruction_orders,
    );
    register_rehydrate_step(
        app,
        "construction.normalize",
        RehydratePhase::DurableNormalize,
        &[],
        &[],
        construction_runtime::normalize_construction_state,
    );
    register_rehydrate_step(
        app,
        "deconstruction.floor-ownership",
        RehydratePhase::DurableNormalize,
        &[],
        &[],
        deconstruction::normalize_completed_floor_ownership,
    );
    register_rehydrate_step(
        app,
        "familiar.settings",
        RehydratePhase::DurableNormalize,
        &[],
        &[],
        normalize_familiar_settings,
    );
    register_rehydrate_step(
        app,
        "power-consumer.policy",
        RehydratePhase::DurableNormalize,
        &[],
        &[],
        rehydrate_power_consumer_policies,
    );
    register_rehydrate_step(
        app,
        "soul-spa.normalize",
        RehydratePhase::DurableNormalize,
        &[],
        &[],
        rehydrate_soul_spas,
    );
    register_rehydrate_step(
        app,
        "stockpile.policy",
        RehydratePhase::DurableNormalize,
        &[],
        &[],
        rehydrate_stockpile_policies,
    );
    register_rehydrate_step(
        app,
        "transport-request.targets",
        RehydratePhase::DurableNormalize,
        &[],
        &[],
        task_runtime::normalize_transport_request_targets,
    );
    register_rehydrate_step(
        app,
        "task-logistics.runtime",
        RehydratePhase::RuntimeNormalize,
        &["transport-request.targets"],
        &[],
        task_runtime::normalize_task_logistics_runtime,
    );
    register_rehydrate_step(
        app,
        "deconstruction.runtime",
        RehydratePhase::RuntimeNormalize,
        &["deconstruction.floor-ownership", "task-logistics.runtime"],
        &[],
        deconstruction::rebuild_deconstruction_runtime,
    );
    register_rehydrate_step(
        app,
        "construction.runtime",
        RehydratePhase::RebuildDerived,
        &["construction.normalize", "presentation.shells"],
        &[],
        construction_runtime::rebuild_construction_runtime,
    );
    register_rehydrate_step(
        app,
        "obstacle.runtime",
        RehydratePhase::RebuildDerived,
        &["construction.runtime"],
        &[],
        rehydrate_obstacle_runtime,
    );
    register_rehydrate_step(
        app,
        "domains.wake",
        RehydratePhase::WakeDomains,
        &["obstacle.runtime"],
        &[],
        wake_domains_after_load,
    );
}

/// Presentation-domain adapter registered by `VisualPlugin`.
pub(crate) fn register_visual_rehydrate_pipeline(app: &mut App) {
    register_candidate_validator(
        app,
        "presentation.spatial-roots",
        candidate::validate_shell_candidate,
    );
    register_live_prerequisite(
        app,
        "presentation.assets-time",
        prerequisites::validate_presentation_prerequisites,
    );
    register_rehydrate_step(
        app,
        "presentation.shells",
        RehydratePhase::AttachShells,
        &["task-logistics.runtime"],
        &["presentation.assets-time"],
        rehydrate_shell_step,
    );
}

pub(super) fn freeze_rehydrate_pipeline(app: &mut App) {
    registry::freeze_rehydrate_registry(app);
}

#[cfg(test)]
pub(crate) fn resolved_rehydrate_plan_names(
    world: &World,
) -> (Vec<&'static str>, Vec<&'static str>, Vec<&'static str>) {
    let plan = world.resource::<ResolvedRehydratePlan>();
    (
        plan.step_names(),
        plan.validator_names(),
        plan.prerequisite_names(),
    )
}

/// Adds operation and work policy defaults to saves created before those
/// components became durable, while preserving every explicitly saved value.
///
/// A save produced by a B2-aware executable can never contain a roster larger
/// than its saved maximum. Treat that state as corruption so the surrounding
/// load transaction restores the previous live world instead of guessing
/// which Soul should be released.
#[cfg(test)]
pub(super) fn rehydrate_familiar_settings(
    world: &mut World,
) -> Result<(), RehydratePrerequisiteError> {
    let settings: Vec<(
        Entity,
        Option<FamiliarOperation>,
        Option<FamiliarPolicy>,
        usize,
    )> = {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&FamiliarOperation>,
            Option<&FamiliarPolicy>,
            Option<&Commanding>,
        ), With<Familiar>>();
        query
            .iter(world)
            .map(|(entity, operation, policy, commanding)| {
                (
                    entity,
                    operation.cloned(),
                    policy.cloned(),
                    commanding.map_or(0, |roster| roster.iter().count()),
                )
            })
            .collect()
    };

    if settings.iter().any(|(_, operation, _, roster_len)| {
        operation
            .as_ref()
            .is_some_and(|operation| operation.max_controlled_soul < *roster_len)
    }) {
        return Err(RehydratePrerequisiteError {
            missing_resources: Vec::new(),
            invalid_conditions: vec![
                "saved FamiliarOperation.max_controlled_soul must cover its Commanding roster",
            ],
        });
    }

    normalize_familiar_settings_from_snapshot(world, settings);

    Ok(())
}

fn normalize_familiar_settings(world: &mut World) {
    let settings: Vec<(
        Entity,
        Option<FamiliarOperation>,
        Option<FamiliarPolicy>,
        usize,
    )> = {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&FamiliarOperation>,
            Option<&FamiliarPolicy>,
            Option<&Commanding>,
        ), With<Familiar>>();
        query
            .iter(world)
            .map(|(entity, operation, policy, commanding)| {
                (
                    entity,
                    operation.cloned(),
                    policy.cloned(),
                    commanding.map_or(0, |roster| roster.iter().count()),
                )
            })
            .collect()
    };
    normalize_familiar_settings_from_snapshot(world, settings);
}

fn normalize_familiar_settings_from_snapshot(
    world: &mut World,
    settings: Vec<(
        Entity,
        Option<FamiliarOperation>,
        Option<FamiliarPolicy>,
        usize,
    )>,
) {
    for (entity, operation, policy, roster_len) in settings {
        let mut entity_mut = world.entity_mut(entity);
        if operation.is_none() {
            let mut operation = FamiliarOperation::default();
            operation.max_controlled_soul = operation.max_controlled_soul.max(roster_len);
            entity_mut.insert(operation);
        }

        match policy {
            Some(policy) => {
                let normalized = policy.clone().normalized();
                if normalized != policy {
                    entity_mut.insert(normalized);
                }
            }
            None => {
                entity_mut.insert(FamiliarPolicy::default());
            }
        }
    }
}

/// Adds the compatibility policy only to ordinary stockpile cells owned by a durable Yard.
///
/// Tank companions lose their runtime-only `BucketStorage` marker in old save bodies, so the
/// positive `BelongsTo -> Yard` ownership check is the migration boundary. Existing policy values
/// are preserved except for the target/capacity invariant.
pub(super) fn rehydrate_stockpile_policies(world: &mut World) {
    let bucket_storages: Vec<Entity> = {
        let mut stockpiles = world.query::<(
            Entity,
            &Stockpile,
            Option<&BelongsTo>,
            Option<&PendingBelongsToBlueprint>,
        )>();
        stockpiles
            .iter(world)
            .filter_map(|(entity, _, owner, pending_owner)| {
                let completed_tank = owner.is_some_and(|owner| {
                    world
                        .get::<Building>(owner.0)
                        .is_some_and(|building| building.kind == BuildingType::Tank)
                });
                let pending_tank = pending_owner.is_some_and(|owner| {
                    world
                        .get::<Blueprint>(owner.0)
                        .is_some_and(|blueprint| blueprint.kind == BuildingType::Tank)
                });
                (completed_tank || pending_tank).then_some(entity)
            })
            .collect()
    };
    for entity in bucket_storages {
        world.entity_mut(entity).insert(BucketStorage);
    }

    let yard_entities: HashSet<Entity> = {
        let mut yards = world.query_filtered::<Entity, With<Yard>>();
        yards.iter(world).collect()
    };
    let missing: Vec<(Entity, usize)> = {
        let mut stockpiles =
            world.query_filtered::<(Entity, &Stockpile, &BelongsTo), Without<StockpilePolicy>>();
        stockpiles
            .iter(world)
            .filter(|(_, _, owner)| yard_entities.contains(&owner.0))
            .map(|(entity, stockpile, _)| (entity, stockpile.capacity))
            .collect()
    };

    for (entity, capacity) in missing {
        world
            .entity_mut(entity)
            .insert(StockpilePolicy::for_capacity(capacity));
    }

    let mut policies = world.query::<(&Stockpile, &mut StockpilePolicy)>();
    for (stockpile, mut policy) in policies.iter_mut(world) {
        let normalized = policy.normalized_for_capacity(stockpile.capacity);
        if *policy != normalized {
            *policy = normalized;
        }
    }
}

/// Normalizes durable Soul Spa controls before the rebuilt energy pipeline can observe them.
pub(super) fn rehydrate_soul_spas(world: &mut World) {
    let mut sites = world.query::<&mut hw_energy::SoulSpaSite>();
    for mut site in sites.iter_mut(world) {
        site.normalize_active_slots();
    }
}

/// Adds the compatibility Normal policy to consumers saved before B3.
pub(super) fn rehydrate_power_consumer_policies(world: &mut World) {
    let missing: Vec<Entity> = {
        let mut consumers = world.query_filtered::<Entity, (
            With<hw_energy::PowerConsumer>,
            Without<hw_energy::PowerConsumerPolicy>,
        )>();
        consumers.iter(world).collect()
    };
    for entity in missing {
        world
            .entity_mut(entity)
            .insert(hw_energy::PowerConsumerPolicy::default());
    }
}

mod construction_runtime;

#[cfg(test)]
use construction_runtime::{normalize_construction_state, rehydrate_construction_runtime};

mod obstacles;

use obstacles::rehydrate_obstacle_runtime;

fn rehydrate_shell_step(world: &mut World) {
    world.resource_scope::<GameAssets, _>(|world, game_assets| {
        world.resource_scope::<Building3dHandles, _>(|world, handles_3d| {
            world.resource_scope::<SoulTaskHandles, _>(|world, soul_handles| {
                rehydrate_shells(world, &game_assets, &handles_3d, &soul_handles);
            });
        });
    });
}

fn wake_domains_after_load(world: &mut World) {
    world.init_resource::<crate::systems::energy::grid_recalc::EnergyUpdateDirty>();
    world
        .resource_mut::<crate::systems::energy::grid_recalc::EnergyUpdateDirty>()
        .request_full_rebuild();
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

    let mut familiars: Vec<(Entity, String, f32, Vec3, bool)> = Vec::new();
    {
        let mut q = world.query_filtered::<
            (Entity, &Familiar, &Transform, Option<&TaskArea>),
            Without<Destination>,
        >();
        for (entity, familiar, transform, task_area) in q.iter(world) {
            familiars.push((
                entity,
                familiar.name.clone(),
                familiar.command_radius,
                transform.translation,
                task_area.is_some(),
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
        let mut q = world.query_filtered::<
            (Entity, &ResourceItem, Option<&LoadedIn>, Option<&StoredIn>),
            Without<Sprite>,
        >();
        for (entity, item, loaded_in, stored_in) in q.iter(world) {
            let hidden =
                loaded_in.is_some() || (item.0 == ResourceType::Water && stored_in.is_some());
            items.push((entity, item.0, hidden));
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

    for (entity, name, command_radius, translation, has_task_area) in familiars {
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
        // ActiveCommand is intentionally runtime-only, while TaskArea is
        // durable. Reconstruct the operational command from that durable
        // capability so F9 does not silently disable every area producer.
        if has_task_area {
            commands.entity(entity).insert(ActiveCommand {
                command: FamiliarCommand::Patrol,
            });
        }
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

    for (entity, resource_type, is_hidden) in items {
        commands
            .entity(entity)
            .insert(item_sprite(resource_type, game_assets, soul_handles));
        // 猫車積載中のアイテムは地面に描画しない（積載ビジュアルは haul 系システムが担う）
        if is_hidden {
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
    let mut souls: Vec<(Entity, Option<SoulIdentity>, String, Vec3, bool)> = Vec::new();
    {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&SoulIdentity>,
            &Transform,
            Option<&RestingIn>,
        ), (With<DamnedSoul>, Without<Destination>)>();
        for (entity, identity, transform, resting_in) in query.iter(world) {
            let translation = transform.translation;
            match identity {
                Some(identity) => souls.push((
                    entity,
                    None,
                    identity.name.clone(),
                    translation,
                    resting_in.is_some(),
                )),
                None => {
                    // 旧形式セーブ（SoulIdentity 未保存）へのフォールバック
                    let identity = SoulIdentity::random();
                    let name = identity.name.clone();
                    souls.push((
                        entity,
                        Some(identity),
                        name,
                        translation,
                        resting_in.is_some(),
                    ));
                }
            }
        }
    }

    let count = souls.len();
    let mut commands = world.commands();
    for (entity, new_identity, name, translation, is_resting) in souls {
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
        if is_resting {
            commands.entity(entity).insert(Visibility::Hidden);
        }
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
