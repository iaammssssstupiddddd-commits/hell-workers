//! Blueprint auto-haul system
//!
//! M3: 設計図への資材運搬を request エンティティ（アンカー側）で発行する。
//! 割り当て時に資材ソースを遅延解決する。

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::events::{DesignationOp, DesignationRequest};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{
    Blueprint, Designation, Priority, SandPile, TaskSlots, TargetBlueprint, WorkType,
};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::ResourceType;
use crate::world::map::{TerrainType, WorldMap};

use crate::systems::spatial::BlueprintSpatialGrid;

/// 設計図への自動資材運搬タスク生成システム
///
/// Blueprint 単位の demand を request エンティティとして発行し、
/// 割り当て時（assign_haul）に資材ソースを遅延解決する。
pub fn blueprint_auto_haul_system(
    mut commands: Commands,
    mut designation_writer: MessageWriter<DesignationRequest>,
    world_map: Res<WorldMap>,
    blueprint_grid: Res<BlueprintSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_bp_requests: Query<(
        Entity,
        &TargetBlueprint,
        &TransportRequest,
        Option<&TaskWorkers>,
    )>,
    q_sand_piles: Query<
        (
            Entity,
            &Transform,
            Option<&Designation>,
            Option<&TaskWorkers>,
        ),
        With<SandPile>,
    >,
    q_task_state: Query<(Option<&Designation>, Option<&TaskWorkers>)>,
) {
    // 1. 集計: 各設計図への「運搬中」の数
    // (BlueprintEntity, ResourceType) -> Count
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();

    // TransportRequest エンティティの TaskWorkers を inflight にカウント
    // M3: AssignedTask ベースのカウントをやめ、TransportRequest / TaskWorkers に一本化する
    for (_, target_bp, req, workers_opt) in q_bp_requests.iter() {
        if matches!(req.kind, TransportRequestKind::DeliverToBlueprint) {
            let count = workers_opt.map(|w| w.len()).unwrap_or(0);
            if count > 0 {
                *in_flight
                    .entry((target_bp.0, req.resource_type))
                    .or_insert(0) += count;
            }
        }
    }

    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| {
            !matches!(active_command.command, FamiliarCommand::Idle)
        })
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    // 2. 各 Blueprint の不足分を計算し、desired_requests に格納
    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();
    let mut collect_sand_pending_fam = std::collections::HashSet::<Entity>::new();

    let mut blueprints_to_process = std::collections::HashSet::new();
    for (_, area) in &active_familiars {
        for &bp_entity in blueprint_grid.get_in_area(area.min, area.max).iter() {
            blueprints_to_process.insert(bp_entity);
        }
    }

    for bp_entity in blueprints_to_process {
        let Ok((_, bp_transform, blueprint, workers_opt)) = q_blueprints.get(bp_entity) else {
            continue;
        };
        let bp_pos = bp_transform.translation.truncate();

        if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
            continue;
        }
        if blueprint.materials_complete() {
            continue;
        }

        let Some((fam_entity, task_area)) = super::find_owner_familiar(bp_pos, &active_familiars)
        else {
            continue;
        };

        for (resource_type, &required) in &blueprint.required_materials {
            let delivered = *blueprint.delivered_materials.get(resource_type).unwrap_or(&0);
            let inflight_count = *in_flight.get(&(bp_entity, *resource_type)).unwrap_or(&0);

            if delivered + inflight_count as u32 >= required {
                continue;
            }

            let needed = required.saturating_sub(delivered + inflight_count as u32);
            desired_requests.insert(
                (bp_entity, *resource_type),
                (fam_entity, needed.max(1), bp_pos),
            );

            if *resource_type == ResourceType::Sand
                && !collect_sand_pending_fam.contains(&fam_entity)
                && let Some(source_entity) = find_available_sand_source(
                    &world_map,
                    Some(task_area),
                    bp_pos,
                    &q_sand_piles,
                    &q_task_state,
                )
            {
                designation_writer.write(DesignationRequest {
                    entity: source_entity,
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
                collect_sand_pending_fam.insert(fam_entity);
            }
        }
    }

    // 3. request エンティティの upsert / cleanup（共通ヘルパー使用）
    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    for (request_entity, target_bp, request, workers_opt) in q_bp_requests.iter() {
        if !matches!(request.kind, TransportRequestKind::DeliverToBlueprint) {
            continue;
        }
        let key = (target_bp.0, request.resource_type);
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

        if let Some((issued_by, slots, bp_pos)) = desired_requests.get(&key) {
            commands.entity(request_entity).try_insert((
                Transform::from_xyz(bp_pos.x, bp_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(0),
                TargetBlueprint(key.0),
                TransportRequest {
                    kind: TransportRequestKind::DeliverToBlueprint,
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
            super::upsert::disable_request(&mut commands, request_entity);
        }
    }

    for (key, (issued_by, slots, bp_pos)) in desired_requests {
        if seen_existing_keys.contains(&key) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::DeliverToBlueprint"),
            Transform::from_xyz(bp_pos.x, bp_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(0),
            TargetBlueprint(key.0),
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
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

fn find_available_sand_source(
    world_map: &WorldMap,
    task_area: Option<&TaskArea>,
    target_pos: Vec2,
    q_sand_piles: &Query<
        (
            Entity,
            &Transform,
            Option<&Designation>,
            Option<&TaskWorkers>,
        ),
        With<SandPile>,
    >,
    q_task_state: &Query<(Option<&Designation>, Option<&TaskWorkers>)>,
) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;

    // 1st pass: TaskArea 内を優先
    for (sp_entity, sp_transform, sp_designation, sp_workers) in q_sand_piles.iter() {
        let pos = sp_transform.translation.truncate();
        if let Some(area) = task_area {
            if !area.contains(pos) {
                continue;
            }
        }
        if sp_designation.is_some() || sp_workers.map(|w| w.len()).unwrap_or(0) > 0 {
            continue;
        }

        let dist_sq = pos.distance_squared(target_pos);
        match best {
            Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
            _ => best = Some((sp_entity, dist_sq)),
        }
    }

    // 2nd pass: 水くみと同様に、TaskArea 内に無ければ全体から取得
    if best.is_none() && task_area.is_some() {
        for (sp_entity, sp_transform, sp_designation, sp_workers) in q_sand_piles.iter() {
            let pos = sp_transform.translation.truncate();
            if sp_designation.is_some() || sp_workers.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            let dist_sq = pos.distance_squared(target_pos);
            match best {
                Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
                _ => best = Some((sp_entity, dist_sq)),
            }
        }
    }

    if let Some((entity, _)) = best {
        return Some(entity);
    }

    let scan_sand_tiles = |area_filter: Option<&TaskArea>| -> Option<(Entity, f32)> {
        let (x0, y0, x1, y1) = if let Some(area) = area_filter {
            let (ax0, ay0) = WorldMap::world_to_grid(area.min);
            let (ax1, ay1) = WorldMap::world_to_grid(area.max);
            (ax0, ay0, ax1, ay1)
        } else {
            (
                0,
                0,
                crate::constants::MAP_WIDTH - 1,
                crate::constants::MAP_HEIGHT - 1,
            )
        };

        let min_x = x0.min(x1);
        let max_x = x0.max(x1);
        let min_y = y0.min(y1);
        let max_y = y0.max(y1);

        let mut best_tile: Option<(Entity, f32)> = None;
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
                if let Some(area) = area_filter {
                    if !area.contains(tile_pos) {
                        continue;
                    }
                }

                let dist_sq = tile_pos.distance_squared(target_pos);
                match best_tile {
                    Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
                    _ => best_tile = Some((tile_entity, dist_sq)),
                }
            }
        }

        best_tile
    };

    let mut best_tile = scan_sand_tiles(task_area);
    if best_tile.is_none() && task_area.is_some() {
        best_tile = scan_sand_tiles(None);
    }

    best_tile.map(|(entity, _)| entity)
}
