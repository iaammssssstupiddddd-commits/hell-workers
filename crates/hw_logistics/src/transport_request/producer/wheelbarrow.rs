//! Wheelbarrow auto-haul producer

use bevy::prelude::*;

use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::{ManagedBy, ParkedAt, PushedBy, TaskWorkers};
use hw_jobs::{Designation, Priority, TaskSlots, WorkType};
use hw_world::zones::AreaBounds;

use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::{ResourceType, Wheelbarrow};

const RETURN_REQUEST_PRIORITY: u32 = 0;
const RETURN_DISTANCE_THRESHOLD_SQ: f32 =
    (hw_core::constants::TILE_SIZE * 1.25) * (hw_core::constants::TILE_SIZE * 1.25);

#[derive(Clone, Copy)]
struct DesiredWheelbarrowRequest {
    issued_by: Entity,
    wb_pos: Vec2,
}

fn to_u32_saturating(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

type WheelbarrowParkedQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static ParkedAt),
    (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>),
>;

pub fn wheelbarrow_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_wheelbarrows: WheelbarrowParkedQuery,
    q_transforms: Query<&Transform>,
    q_wb_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
) {
    let active_familiars: Vec<(Entity, AreaBounds)> = q_familiars
        .iter()
        .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
        .map(|(e, _, a)| (e, a.bounds()))
        .collect();

    let mut desired_return_requests =
        std::collections::HashMap::<Entity, DesiredWheelbarrowRequest>::new();

    for (wb_entity, wb_transform, parked_at) in q_wheelbarrows.iter() {
        let wb_pos = wb_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner(wb_pos, &active_familiars) else {
            continue;
        };

        let desired = DesiredWheelbarrowRequest {
            issued_by: fam_entity,
            wb_pos,
        };

        let needs_return = q_transforms
            .get(parked_at.0)
            .ok()
            .is_some_and(|parking_transform| {
                parking_transform
                    .translation
                    .truncate()
                    .distance_squared(wb_pos)
                    > RETURN_DISTANCE_THRESHOLD_SQ
            });
        if needs_return {
            desired_return_requests.insert(wb_entity, desired);
        }
    }

    let mut seen_return = std::collections::HashSet::new();
    for (req_entity, req, workers_opt) in q_wb_requests.iter() {
        let wb_entity = req.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        let inflight = to_u32_saturating(workers);

        match req.kind {
            TransportRequestKind::BatchWheelbarrow => {
                // BatchWheelbarrow は現在ファミリア AI に処理されないため生成しない。
                // 残存エンティティをクリーンアップする。
                if workers == 0 {
                    commands.entity(req_entity).try_despawn();
                }
            }
            TransportRequestKind::ReturnWheelbarrow => {
                if !super::upsert::process_duplicate_key(
                    &mut commands,
                    req_entity,
                    workers,
                    &mut seen_return,
                    wb_entity,
                ) {
                    continue;
                }

                if let Some(desired) = desired_return_requests.get(&wb_entity) {
                    commands.entity(req_entity).try_insert((
                        Transform::from_xyz(desired.wb_pos.x, desired.wb_pos.y, 0.0),
                        Visibility::Hidden,
                        Designation {
                            work_type: WorkType::WheelbarrowHaul,
                        },
                        ManagedBy(desired.issued_by),
                        TaskSlots::new(1),
                        Priority(RETURN_REQUEST_PRIORITY),
                        TransportRequest {
                            kind: TransportRequestKind::ReturnWheelbarrow,
                            anchor: wb_entity,
                            resource_type: ResourceType::Wheelbarrow,
                            issued_by: desired.issued_by,
                            priority: TransportPriority::Low,
                            stockpile_group: vec![],
                        },
                        TransportDemand {
                            desired_slots: 1,
                            inflight,
                        },
                        TransportRequestState::Pending,
                        TransportPolicy::default(),
                    ));
                } else if workers == 0 {
                    commands.entity(req_entity).try_despawn();
                } else {
                    super::upsert::disable_request(&mut commands, req_entity);
                    commands.entity(req_entity).try_insert(TransportDemand {
                        desired_slots: 0,
                        inflight,
                    });
                }
            }
            _ => {}
        }
    }

    for (wb_entity, desired) in desired_return_requests {
        if seen_return.contains(&wb_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::ReturnWheelbarrow"),
            Transform::from_xyz(desired.wb_pos.x, desired.wb_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::WheelbarrowHaul,
            },
            ManagedBy(desired.issued_by),
            TaskSlots::new(1),
            Priority(RETURN_REQUEST_PRIORITY),
            TransportRequest {
                kind: TransportRequestKind::ReturnWheelbarrow,
                anchor: wb_entity,
                resource_type: ResourceType::Wheelbarrow,
                issued_by: desired.issued_by,
                priority: TransportPriority::Low,
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
