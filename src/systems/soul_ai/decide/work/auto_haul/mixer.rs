//! MudMixer auto-haul system
//!
//! Automatically creates haul tasks for materials needed by MudMixer.

use bevy::prelude::*;

use crate::constants::{BUCKET_CAPACITY, MUD_MIXER_CAPACITY, TILE_SIZE};
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::events::{
    DesignationOp, DesignationRequest, ResourceReservationOp, ResourceReservationRequest,
};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::{
    Designation, MixerHaulRequest, MudMixerStorage, Priority, TargetMixer, TaskSlots, WorkType,
};
use crate::systems::logistics::{ReservedForTask, ResourceItem, ResourceType, Stockpile};
use crate::systems::soul_ai::decide::work::auto_haul::ItemReservations;

/// MudMixer への自動資材運搬タスク生成システム
pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    mut designation_writer: MessageWriter<DesignationRequest>,
    haul_cache: Res<SharedResourceCache>,
    mut item_reservations: ResMut<ItemReservations>,
    mut reservation_writer: MessageWriter<ResourceReservationRequest>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_mixers: Query<(Entity, &Transform, &MudMixerStorage, Option<&TaskWorkers>)>,
    q_mixer_requests: Query<(
        Entity,
        &TargetMixer,
        &MixerHaulRequest,
        Option<&Designation>,
        Option<&TaskWorkers>,
    )>,
    q_stockpiles_detailed: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_resources_with_belongs: Query<(
        Entity,
        &Transform,
        &Visibility,
        &ResourceItem,
        Option<&crate::systems::logistics::BelongsTo>,
        Option<&crate::relationships::StoredIn>,
        Option<&ReservedForTask>,
        Option<&Designation>,
        Option<&TaskWorkers>,
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
) {
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| {
            !matches!(active_command.command, FamiliarCommand::Idle)
        })
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    let mut already_assigned_this_frame = std::collections::HashSet::new();
    let mut mixer_reservation_delta =
        std::collections::HashMap::<(Entity, ResourceType), usize>::new();

    // (mixer, resource_type) -> (issued_by, desired_slots, mixer_pos)
    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();
    let mut active_mixers = std::collections::HashSet::<Entity>::new();

    for (mixer_entity, mixer_transform, storage, _workers_opt) in q_mixers.iter() {
        active_mixers.insert(mixer_entity);

        let mixer_pos = mixer_transform.translation.truncate();
        let Some((fam_entity, task_area)) = find_owner_familiar(mixer_pos, &active_familiars) else {
            continue;
        };

        let other_areas: Vec<&TaskArea> = active_familiars
            .iter()
            .filter(|(entity, _)| *entity != fam_entity)
            .map(|(_, area)| area)
            .collect();

        // -----------------------------------------------------------------
        // 固体原料は request タスクを発行（ソースは割り当て時に遅延解決）
        // -----------------------------------------------------------------
        for resource_type in [ResourceType::Sand, ResourceType::Rock] {
            let current = match resource_type {
                ResourceType::Sand => storage.sand,
                ResourceType::Rock => storage.rock,
                _ => 0,
            };

            let inflight = haul_cache.get_mixer_destination_reservation(mixer_entity, resource_type)
                + mixer_reservation_delta
                    .get(&(mixer_entity, resource_type))
                    .cloned()
                    .unwrap_or(0);

            let needed = MUD_MIXER_CAPACITY.saturating_sub(current + inflight as u32);
            if needed > 0 {
                desired_requests.insert(
                    (mixer_entity, resource_type),
                    (fam_entity, needed.max(1), mixer_pos),
                );
            }
        }

        // --- 砂採取タスクの自動発行 ---
        let sand_inflight = haul_cache.get_mixer_destination_reservation(mixer_entity, ResourceType::Sand)
            + mixer_reservation_delta
                .get(&(mixer_entity, ResourceType::Sand))
                .cloned()
                .unwrap_or(0);
        if storage.sand + (sand_inflight as u32) < 2 {
            for (sp_entity, sp_transform, sp_designation, sp_workers) in q_sand_piles.iter() {
                let dist = sp_transform.translation.truncate().distance(mixer_pos);
                if dist < TILE_SIZE * 3.0 && task_area.contains(sp_transform.translation.truncate()) {
                    let has_designation = sp_designation.is_some() || sp_workers.is_some();
                    if !has_designation {
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
                            "AUTO_HAUL_MIXER: Issued CollectSand for Mixer {:?}",
                            mixer_entity
                        );
                        break;
                    }
                }
            }
        }

        // --- 水の自動リクエスト（従来方式） ---
        let water_inflight_tasks = haul_cache
            .get_mixer_destination_reservation(mixer_entity, ResourceType::Water)
            + mixer_reservation_delta
                .get(&(mixer_entity, ResourceType::Water))
                .cloned()
                .unwrap_or(0);
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

        if water_current < water_capacity && water_current + water_inflight <= issue_threshold {
            let mut tank_candidates = Vec::new();

            for (stock_entity, stock_transform, stock, stored_opt) in q_stockpiles_detailed.iter() {
                if stock.resource_type != Some(ResourceType::Water) {
                    continue;
                }

                let tank_pos = stock_transform.translation.truncate();
                if other_areas.iter().any(|area| area.contains(tank_pos)) {
                    continue;
                }

                let water_count = stored_opt.map(|s| s.len()).unwrap_or(0);
                if water_count >= BUCKET_CAPACITY as usize {
                    let dist_sq = tank_pos.distance_squared(mixer_pos);
                    tank_candidates.push((stock_entity, dist_sq));
                }
            }

            tank_candidates
                .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let tank_with_water = tank_candidates.first().map(|(e, _)| *e);

            if let Some(tank_entity) = tank_with_water {
                let mut bucket_candidates = Vec::new();

                for (
                    e,
                    transform,
                    vis,
                    res_item,
                    belongs_opt,
                    stored_in_opt,
                    reserved_opt,
                    designation,
                    workers,
                ) in q_resources_with_belongs.iter()
                {
                    if *vis == Visibility::Hidden || workers.is_some() {
                        continue;
                    }
                    if !matches!(
                        res_item.0,
                        ResourceType::BucketEmpty | ResourceType::BucketWater
                    ) {
                        continue;
                    }
                    if reserved_opt.is_some() {
                        continue;
                    }
                    if already_assigned_this_frame.contains(&e) {
                        continue;
                    }
                    if item_reservations.0.contains(&e) {
                        continue;
                    }
                    if designation.is_some() {
                        continue;
                    }

                    if let Some(stored_in) = stored_in_opt {
                        if let Ok((_, stock_transform, _, _)) = q_stockpiles_detailed.get(stored_in.0)
                        {
                            let stock_pos = stock_transform.translation.truncate();
                            if other_areas.iter().any(|area| area.contains(stock_pos)) {
                                continue;
                            }
                        }
                    }

                    if let Some(belongs) = belongs_opt {
                        if belongs.0 == tank_entity {
                            let item_pos = transform.translation.truncate();
                            let dist_sq = item_pos.distance_squared(mixer_pos);
                            bucket_candidates.push((e, dist_sq, res_item.0));
                        }
                    }
                }

                bucket_candidates.sort_by(|a, b| {
                    let type_order_a = if a.2 == ResourceType::BucketEmpty { 0 } else { 1 };
                    let type_order_b = if b.2 == ResourceType::BucketEmpty { 0 } else { 1 };
                    match type_order_a.cmp(&type_order_b) {
                        std::cmp::Ordering::Equal => {
                            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        other => other,
                    }
                });

                if let Some((bucket_entity, _, _)) = bucket_candidates.first() {
                    designation_writer.write(DesignationRequest {
                        entity: *bucket_entity,
                        operation: DesignationOp::Issue {
                            work_type: WorkType::HaulWaterToMixer,
                            issued_by: fam_entity,
                            task_slots: 1,
                            priority: Some(6),
                            target_blueprint: None,
                            target_mixer: Some(mixer_entity),
                            reserved_for_task: true,
                        },
                    });
                    item_reservations.0.insert(*bucket_entity);
                    *mixer_reservation_delta
                        .entry((mixer_entity, ResourceType::Water))
                        .or_insert(0) += 1;
                    reservation_writer.write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReserveMixerDestination {
                            target: mixer_entity,
                            resource_type: ResourceType::Water,
                        },
                    });
                    already_assigned_this_frame.insert(*bucket_entity);
                    info!(
                        "AUTO_HAUL_MIXER: Issued HaulWaterToMixer for bucket {:?} (Mixer {:?})",
                        bucket_entity, mixer_entity
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // request エンティティを upsert / cleanup
    // -----------------------------------------------------------------
    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    for (request_entity, target_mixer, request, _designation_opt, workers_opt) in q_mixer_requests.iter()
    {
        let key = (target_mixer.0, request.resource_type);
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !seen_existing_keys.insert(key) {
            if workers == 0 {
                commands.entity(request_entity).despawn();
            }
            continue;
        }

        if let Some((issued_by, slots, mixer_pos)) = desired_requests.get(&key) {
            commands.entity(request_entity).insert((
                Transform::from_xyz(mixer_pos.x, mixer_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::HaulToMixer,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(5),
                TargetMixer(key.0),
                MixerHaulRequest {
                    resource_type: key.1,
                },
            ));
            continue;
        }

        if workers == 0 {
            if !active_mixers.contains(&target_mixer.0) {
                commands.entity(request_entity).despawn();
            } else {
                commands
                    .entity(request_entity)
                    .remove::<Designation>()
                    .remove::<TaskSlots>()
                    .remove::<Priority>();
            }
        }
    }

    for (key, (issued_by, slots, mixer_pos)) in desired_requests {
        if seen_existing_keys.contains(&key) {
            continue;
        }

        commands.spawn((
            Name::new("MixerHaulRequest"),
            Transform::from_xyz(mixer_pos.x, mixer_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::HaulToMixer,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(5),
            TargetMixer(key.0),
            MixerHaulRequest {
                resource_type: key.1,
            },
        ));
    }
}

fn find_owner_familiar(mixer_pos: Vec2, familiars: &[(Entity, TaskArea)]) -> Option<(Entity, &TaskArea)> {
    familiars
        .iter()
        .filter(|(_, area)| area.contains(mixer_pos))
        .min_by(|(_, area1), (_, area2)| {
            let d1 = area1.center().distance_squared(mixer_pos);
            let d2 = area2.center().distance_squared(mixer_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, area)| (*entity, area))
}
