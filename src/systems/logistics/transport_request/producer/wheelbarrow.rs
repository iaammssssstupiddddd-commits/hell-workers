//! Wheelbarrow auto-haul producer
//!
//! 利用可能な手押し車を検知し、`BatchWheelbarrow` リクエストを発行します。

use bevy::prelude::*;
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{ParkedAt, PushedBy, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{ResourceType, Wheelbarrow};

/// 利用可能な手押し車を検知し、一括運搬リクエストを発行するシステム
pub fn wheelbarrow_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_wheelbarrows: Query<(Entity, &Transform), (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>)>,
    q_wb_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
) {
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
        .map(|(e, _, a)| (e, a.clone()))
        .collect();

    // (wheelbarrow_entity) -> (issued_by, wb_pos)
    let mut desired_requests = std::collections::HashMap::<Entity, (Entity, Vec2)>::new();

    for (wb_entity, wb_transform) in q_wheelbarrows.iter() {
        let wb_pos = wb_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner_familiar(wb_pos, &active_familiars) else {
            continue;
        };

        desired_requests.insert(wb_entity, (fam_entity, wb_pos));
    }

    let mut seen = std::collections::HashSet::new();
    for (req_entity, req, workers_opt) in q_wb_requests.iter() {
        if req.kind != TransportRequestKind::BatchWheelbarrow {
            continue;
        }
        let wb_entity = req.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(
            &mut commands,
            req_entity,
            workers,
            &mut seen,
            wb_entity,
        ) {
            continue;
        }

        if let Some((issued_by, wb_pos)) = desired_requests.get(&wb_entity) {
            // Update: 位置が移動している可能性があるため Transform を更新
            commands.entity(req_entity).insert((
                Transform::from_xyz(wb_pos.x, wb_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::WheelbarrowHaul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(1),
                Priority(0),
                TransportRequest {
                    kind: TransportRequestKind::BatchWheelbarrow,
                    anchor: wb_entity,
                    resource_type: ResourceType::Wheelbarrow,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: 1,
                    inflight: 0,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
        } else if workers == 0 {
            // 需要がなくなった
            commands.entity(req_entity).despawn();
        }
    }

    // New spawns
    for (wb_entity, (issued_by, wb_pos)) in desired_requests {
        if seen.contains(&wb_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::BatchWheelbarrow"),
            Transform::from_xyz(wb_pos.x, wb_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::WheelbarrowHaul,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(1),
            Priority(0),
            TransportRequest {
                kind: TransportRequestKind::BatchWheelbarrow,
                anchor: wb_entity,
                resource_type: ResourceType::Wheelbarrow,
                issued_by,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            },
            TransportDemand {
                desired_slots: 1,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}
