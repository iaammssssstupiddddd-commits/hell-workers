//! MudMixer auto-haul system
//!
//! Automatically creates haul tasks for materials needed by MudMixer.

use bevy::prelude::*;

use crate::constants::{BUCKET_CAPACITY, MUD_MIXER_CAPACITY};
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::events::{DesignationOp, DesignationRequest};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::{
    Designation, MudMixerStorage, Priority, TargetMixer, TaskSlots, WorkType,
};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{ResourceType, Stockpile};
use crate::world::map::{TerrainType, WorldMap};

/// MudMixer への自動資材運搬タスク生成システム
pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    mut designation_writer: MessageWriter<DesignationRequest>,
    haul_cache: Res<SharedResourceCache>,
    world_map: Res<WorldMap>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_mixers: Query<(Entity, &Transform, &MudMixerStorage, Option<&TaskWorkers>)>,
    q_mixer_requests: Query<(
        Entity,
        &TargetMixer,
        &TransportRequest,
        Option<&Designation>,
        Option<&TaskWorkers>,
    )>,
    q_stockpiles_detailed: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_sand_piles: Query<
        (
            Entity,
            &Transform,
            Option<&Designation>,
            Option<&TaskWorkers>,
        ),
        With<crate::systems::jobs::SandPile>,
    >,
    q_task_state: Query<(Option<&Designation>, Option<&TaskWorkers>)>,
    q_requests_for_demand: Query<(&TransportRequest, Option<&TaskWorkers>, Option<&TransportDemand>)>,
) {
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    let mut familiar_with_collect_sand_demand = std::collections::HashSet::<Entity>::new();
    for (request, workers_opt, demand_opt) in q_requests_for_demand.iter() {
        if !request_is_collect_sand_demand(request) {
            continue;
        }

        let desired_slots = demand_opt.map(|d| d.desired_slots).unwrap_or(0);
        let workers = workers_opt.map(|w| w.len() as u32).unwrap_or(0);
        if desired_slots == 0 && workers == 0 {
            continue;
        }

        familiar_with_collect_sand_demand.insert(request.issued_by);
    }

    // (mixer, resource_type) -> (issued_by, desired_slots, mixer_pos)
    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();
    let mut active_mixers = std::collections::HashSet::<Entity>::new();

    for (mixer_entity, mixer_transform, storage, _workers_opt) in q_mixers.iter() {
        active_mixers.insert(mixer_entity);

        let mixer_pos = mixer_transform.translation.truncate();
        let Some((fam_entity, task_area)) =
            super::find_owner_familiar(mixer_pos, &active_familiars)
        else {
            continue;
        };

        // -----------------------------------------------------------------
        // 固体原料は request タスクを発行（ソースは割り当て時に遅延解決）
        // -----------------------------------------------------------------
        for resource_type in [ResourceType::Sand, ResourceType::Rock] {
            let current = match resource_type {
                ResourceType::Sand => storage.sand,
                ResourceType::Rock => storage.rock,
                _ => 0,
            };

            let _inflight =
                haul_cache.get_mixer_destination_reservation(mixer_entity, resource_type);

            let needed = MUD_MIXER_CAPACITY.saturating_sub(current);
            if needed > 0 {
                desired_requests.insert(
                    (mixer_entity, resource_type),
                    (fam_entity, needed.max(1), mixer_pos),
                );
            }
        }

        // --- 砂採取タスクの自動発行 ---
        let sand_inflight =
            haul_cache.get_mixer_destination_reservation(mixer_entity, ResourceType::Sand);
        let has_collect_sand_demand = familiar_with_collect_sand_demand.contains(&fam_entity);
        if has_collect_sand_demand && storage.sand + (sand_inflight as u32) < 2 {
            let mut issued_collect_sand = false;

            for (sp_entity, sp_transform, sp_designation, sp_workers) in q_sand_piles.iter() {
                if !task_area.contains(sp_transform.translation.truncate()) {
                    continue;
                }
                if sp_designation.is_some() || sp_workers.map(|w| w.len()).unwrap_or(0) > 0 {
                    continue;
                }

                designation_writer.write(DesignationRequest {
                    entity: sp_entity,
                    operation: DesignationOp::Issue {
                        work_type: WorkType::CollectSand,
                        issued_by: fam_entity,
                        task_slots: 1,
                        priority: Some(4),
                        target_blueprint: None,
                        target_mixer: None,
                        reserved_for_task: false,
                    },
                });
                info!(
                    "AUTO_HAUL_MIXER: Issued CollectSand from SandPile {:?} for Mixer {:?}",
                    sp_entity, mixer_entity
                );
                issued_collect_sand = true;
                break;
            }

            if !issued_collect_sand {
                if let Some(sand_tile_entity) =
                    find_available_sand_tile(&world_map, &task_area, mixer_pos, &q_task_state)
                {
                    designation_writer.write(DesignationRequest {
                        entity: sand_tile_entity,
                        operation: DesignationOp::Issue {
                            work_type: WorkType::CollectSand,
                            issued_by: fam_entity,
                            task_slots: 1,
                            priority: Some(4),
                            target_blueprint: None,
                            target_mixer: None,
                            reserved_for_task: false,
                        },
                    });
                    info!(
                        "AUTO_HAUL_MIXER: Issued CollectSand from beach tile {:?} for Mixer {:?}",
                        sand_tile_entity, mixer_entity
                    );
                }
            }
        }

        // --- 水の自動リクエスト（従来方式） ---
        let water_inflight_tasks =
            haul_cache.get_mixer_destination_reservation(mixer_entity, ResourceType::Water);
        let water_inflight = (water_inflight_tasks as u32) * BUCKET_CAPACITY;

        let (water_current, water_capacity) =
            if let Ok((_, _, stock, stored_opt)) = q_stockpiles_detailed.get(mixer_entity) {
                if stock.resource_type == Some(ResourceType::Water) {
                    (
                        stored_opt.map(|s| s.len()).unwrap_or(0) as u32,
                        stock.capacity as u32,
                    )
                } else {
                    (0, MUD_MIXER_CAPACITY)
                }
            } else {
                (0, MUD_MIXER_CAPACITY)
            };
        let issue_threshold = water_capacity.saturating_sub(BUCKET_CAPACITY);

        // M5: 水は request エンティティ化（割り当て時にタンク・バケツを遅延解決）
        if water_current < water_capacity && water_current + water_inflight <= issue_threshold {
            desired_requests.insert(
                (mixer_entity, ResourceType::Water),
                (fam_entity, 1, mixer_pos),
            );
        }
    }

    // -----------------------------------------------------------------
    // request エンティティを upsert / cleanup（共通ヘルパー使用）
    // -----------------------------------------------------------------
    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    for (request_entity, target_mixer, request, _designation_opt, workers_opt) in
        q_mixer_requests.iter()
    {
        // 固体・水 request エンティティを対象
        let key = (target_mixer.0, request.resource_type);
        let is_water = request.kind == TransportRequestKind::DeliverWaterToMixer;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing_keys,
            key,
        ) {
            continue;
        }

        if let Some((issued_by, slots, mixer_pos)) = desired_requests.get(&key) {
            let (work_type, kind) = if is_water {
                (
                    WorkType::HaulWaterToMixer,
                    TransportRequestKind::DeliverWaterToMixer,
                )
            } else {
                (
                    WorkType::HaulToMixer,
                    TransportRequestKind::DeliverToMixerSolid,
                )
            };
            commands.entity(request_entity).try_insert((
                Transform::from_xyz(mixer_pos.x, mixer_pos.y, 0.0),
                Visibility::Hidden,
                Designation { work_type },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(5),
                TargetMixer(key.0),
                TransportRequest {
                    kind,
                    anchor: key.0,
                    resource_type: key.1,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: *slots,
                    inflight: 0,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
            continue;
        }

        if workers == 0 {
            if !active_mixers.contains(&target_mixer.0) {
                commands.entity(request_entity).try_despawn();
            } else {
                super::upsert::disable_request(&mut commands, request_entity);
            }
        }
    }

    for (key, (issued_by, slots, mixer_pos)) in desired_requests {
        if seen_existing_keys.contains(&key) {
            continue;
        }

        let (work_type, kind, name) = if key.1 == ResourceType::Water {
            (
                WorkType::HaulWaterToMixer,
                TransportRequestKind::DeliverWaterToMixer,
                "TransportRequest::DeliverWaterToMixer",
            )
        } else {
            (
                WorkType::HaulToMixer,
                TransportRequestKind::DeliverToMixerSolid,
                "TransportRequest::DeliverToMixerSolid",
            )
        };

        commands.spawn((
            Name::new(name),
            Transform::from_xyz(mixer_pos.x, mixer_pos.y, 0.0),
            Visibility::Hidden,
            Designation { work_type },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(5),
            TargetMixer(key.0),
            TransportRequest {
                kind,
                anchor: key.0,
                resource_type: key.1,
                issued_by,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            },
            TransportDemand {
                desired_slots: slots,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}

fn request_is_collect_sand_demand(request: &TransportRequest) -> bool {
    matches!(
        (request.kind, request.resource_type),
        (TransportRequestKind::DeliverToBlueprint, ResourceType::Sand)
            | (TransportRequestKind::DeliverToBlueprint, ResourceType::StasisMud)
            | (
                TransportRequestKind::DeliverToFloorConstruction,
                ResourceType::StasisMud
            )
            | (
                TransportRequestKind::DeliverToWallConstruction,
                ResourceType::StasisMud
            )
            | (
                TransportRequestKind::DeliverToProvisionalWall,
                ResourceType::StasisMud
            )
    )
}

fn find_available_sand_tile(
    world_map: &WorldMap,
    task_area: &TaskArea,
    mixer_pos: Vec2,
    q_task_state: &Query<(Option<&Designation>, Option<&TaskWorkers>)>,
) -> Option<Entity> {
    let (x0, y0) = WorldMap::world_to_grid(task_area.min);
    let (x1, y1) = WorldMap::world_to_grid(task_area.max);

    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    let mut best: Option<(Entity, f32)> = None;

    for gy in min_y..=max_y {
        for gx in min_x..=max_x {
            let Some(idx) = world_map.pos_to_idx(gx, gy) else {
                continue;
            };
            if world_map.tiles[idx] != TerrainType::Sand {
                continue;
            }

            let Some(tile_entity) = world_map.tile_entities[idx] else {
                continue;
            };
            let Ok((designation, workers)) = q_task_state.get(tile_entity) else {
                continue;
            };
            if designation.is_some() || workers.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            let tile_pos = WorldMap::grid_to_world(gx, gy);
            if !task_area.contains(tile_pos) {
                continue;
            }

            let dist_sq = tile_pos.distance_squared(mixer_pos);
            match best {
                Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
                _ => best = Some((tile_entity, dist_sq)),
            }
        }
    }

    best.map(|(entity, _)| entity)
}
