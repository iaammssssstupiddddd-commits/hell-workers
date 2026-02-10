//! Blueprint auto-haul system
//!
//! M3: 設計図への資材運搬を request エンティティ（アンカー側）で発行する。
//! 割り当て時に資材ソースを遅延解決する。

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, Designation, Priority, TaskSlots, TargetBlueprint, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::ResourceType;

use crate::systems::spatial::BlueprintSpatialGrid;

/// 設計図への自動資材運搬タスク生成システム
///
/// Blueprint 単位の demand を request エンティティとして発行し、
/// 割り当て時（assign_haul）に資材ソースを遅延解決する。
pub fn blueprint_auto_haul_system(
    mut commands: Commands,
    blueprint_grid: Res<BlueprintSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_bp_requests: Query<(
        Entity,
        &TargetBlueprint,
        &TransportRequest,
        Option<&TaskWorkers>,
    )>,
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

        let Some((fam_entity, _)) = super::find_owner_familiar(bp_pos, &active_familiars) else {
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
        }
    }

    // 3. request エンティティの upsert / cleanup
    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    for (request_entity, target_bp, request, workers_opt) in q_bp_requests.iter() {
        if !matches!(request.kind, TransportRequestKind::DeliverToBlueprint) {
            continue;
        }
        let key = (target_bp.0, request.resource_type);
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !seen_existing_keys.insert(key) {
            if workers == 0 {
                commands.entity(request_entity).despawn();
            }
            continue;
        }

        if let Some((issued_by, slots, bp_pos)) = desired_requests.get(&key) {
            commands.entity(request_entity).insert((
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
            commands
                .entity(request_entity)
                .remove::<Designation>()
                .remove::<TaskSlots>()
                .remove::<Priority>();
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

