//! Tank water request system
//!
//! Monitors tank storage levels and issues water gathering tasks when tanks are low.

use crate::constants::BUCKET_CAPACITY;
use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{ResourceType, Stockpile};

/// タンクの貯蔵量を監視し、空きがあれば TransportRequest を発行するシステム
pub fn tank_water_request_system(
    mut commands: Commands,
    haul_cache: Res<SharedResourceCache>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    // タンク自体の在庫状況（Water を貯める Stockpile）
    q_tanks: Query<(Entity, &Transform, &Stockpile, Option<&StoredItems>)>,
    q_tank_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
) {
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| {
            !matches!(active_command.command, FamiliarCommand::Idle)
        })
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    // (tank_entity) -> (issued_by, needed_slots, tank_pos)
    let mut desired_requests =
        std::collections::HashMap::<Entity, (Entity, u32, Vec2)>::new();

    for (tank_entity, tank_transform, tank_stock, stored_opt) in q_tanks.iter() {
        // 水タンク以外はスキップ
        if tank_stock.resource_type != Some(ResourceType::Water) {
            continue;
        }

        let tank_pos = tank_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner_familiar(tank_pos, &active_familiars) else {
            continue;
        };

        let current_water = stored_opt.map(|s| s.len()).unwrap_or(0);
        let reserved_water_tasks = haul_cache.get_destination_reservation(tank_entity);
        let total_water = (current_water as u32) + (reserved_water_tasks as u32 * BUCKET_CAPACITY);

        if total_water < tank_stock.capacity as u32 {
            let needed_water = tank_stock.capacity as u32 - total_water;
            let needed_tasks = (needed_water + BUCKET_CAPACITY - 1) / BUCKET_CAPACITY;
            
            desired_requests.insert(
                tank_entity,
                (fam_entity, needed_tasks, tank_pos),
            );
        }
    }

    // -----------------------------------------------------------------
    // request エンティティを upsert / cleanup（共通ヘルパー使用）
    // -----------------------------------------------------------------
    let mut seen_existing = std::collections::HashSet::<Entity>::new();

    for (request_entity, request, workers_opt) in q_tank_requests.iter() {
        if request.kind != TransportRequestKind::GatherWaterToTank {
            continue;
        }
        let tank_entity = request.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing,
            tank_entity,
        ) {
            continue;
        }

        if let Some((issued_by, slots, tank_pos)) = desired_requests.get(&tank_entity) {
            commands.entity(request_entity).insert((
                Transform::from_xyz(tank_pos.x, tank_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::GatherWater,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(3),
                TransportRequest {
                    kind: TransportRequestKind::GatherWaterToTank,
                    anchor: tank_entity,
                    resource_type: ResourceType::Water,
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

    for (tank_entity, (issued_by, slots, tank_pos)) in desired_requests {
        if seen_existing.contains(&tank_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::GatherWaterToTank"),
            Transform::from_xyz(tank_pos.x, tank_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::GatherWater,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(3),
            TransportRequest {
                kind: TransportRequestKind::GatherWaterToTank,
                anchor: tank_entity,
                resource_type: ResourceType::Water,
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
