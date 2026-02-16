//! Stockpile 統合 producer
//!
//! グループ内で同種の資材が複数セルに分散している場合、
//! 少ないセルに集約するための TransportRequest を発行する。

use bevy::prelude::*;
use std::collections::HashMap;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    ManualTransportRequest, TransportDemand, TransportPolicy, TransportPriority, TransportRequest,
    TransportRequestKind, TransportRequestState,
};
use crate::systems::logistics::{BucketStorage, ResourceType, Stockpile};
use crate::systems::spatial::StockpileSpatialGrid;

use super::stockpile_group::build_stockpile_groups;

/// セルの資材情報
struct CellInfo {
    entity: Entity,
    resource_type: Option<ResourceType>,
    stored: usize,
    capacity: usize,
}

/// 統合 producer システム
///
/// グループ内の同一 ResourceType が 2+ セルに分散 → 統合リクエスト生成。
/// 通常 Haul より低い Priority(-1) で、通常搬入が完了してから統合。
pub fn stockpile_consolidation_producer_system(
    mut commands: Commands,
    stockpile_grid: Res<StockpileSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&StoredItems>,
        Option<&BucketStorage>,
    )>,
    q_existing_requests: Query<
        (Entity, &TransportRequest, Option<&TaskWorkers>),
        Without<ManualTransportRequest>,
    >,
) {
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
        .map(|(e, _, a)| (e, a.clone()))
        .collect();

    let groups = build_stockpile_groups(&stockpile_grid, &active_familiars, &q_stockpiles);

    // 統合候補を算出
    let mut desired_requests =
        HashMap::<(Entity, ResourceType), (Entity, Vec<Entity>, usize, Vec2)>::new();

    for group in &groups {
        // セル情報を収集
        let mut cells: Vec<CellInfo> = Vec::new();
        for &cell in &group.cells {
            let Ok((entity, _, stockpile, stored_opt, bucket_opt)) = q_stockpiles.get(cell) else {
                continue;
            };
            if bucket_opt.is_some() {
                continue;
            }
            cells.push(CellInfo {
                entity,
                resource_type: stockpile.resource_type,
                stored: stored_opt.map(|s| s.len()).unwrap_or(0),
                capacity: stockpile.capacity,
            });
        }

        // ResourceType ごとにセルをグループ化
        let mut by_type: HashMap<ResourceType, Vec<&CellInfo>> = HashMap::new();
        for cell in &cells {
            if let Some(rt) = cell.resource_type {
                if cell.stored > 0 {
                    by_type.entry(rt).or_default().push(cell);
                }
            }
        }

        // 2+セルに分散しているタイプについて統合候補を作成
        for (resource_type, mut type_cells) in by_type {
            if type_cells.len() < 2 {
                continue;
            }

            // 格納数降順ソート（最多格納がレシーバー候補）
            type_cells.sort_by(|a, b| b.stored.cmp(&a.stored));

            // 貪欲法: レシーバーにドナーの分を詰めていく
            let mut receivers_used = 0usize;
            let total_items: usize = type_cells.iter().map(|c| c.stored).sum();
            let mut remaining = total_items;

            // 何セルあれば全アイテムが収まるか計算
            for cell in &type_cells {
                if remaining == 0 {
                    break;
                }
                let fit = remaining.min(cell.capacity);
                remaining -= fit;
                receivers_used += 1;
            }

            let freed_cells = type_cells.len() - receivers_used;
            if freed_cells == 0 {
                continue;
            }

            // レシーバー = 最初に見つかった満杯でないセル、ドナー = それ以外
            let receiver_idx = type_cells.iter().position(|c| c.stored < c.capacity);
            let Some(r_idx) = receiver_idx else {
                continue; // 全セル満杯
            };

            let receiver = type_cells[r_idx].entity;
            let donor_cells: Vec<Entity> = type_cells
                .iter()
                .enumerate()
                .filter(|(i, c)| *i != r_idx && c.stored > 0)
                .map(|(_, c)| c.entity)
                .collect();

            if donor_cells.is_empty() {
                continue;
            }

            // 移動数 = レシーバーの空き容量（実際に移動可能な量）
            let receiver_free = type_cells[r_idx]
                .capacity
                .saturating_sub(type_cells[r_idx].stored);
            let donor_total: usize = donor_cells
                .iter()
                .filter_map(|&e| type_cells.iter().find(|c| c.entity == e))
                .map(|c| c.stored)
                .sum();
            let transfer_count = receiver_free.min(donor_total);

            if transfer_count == 0 {
                continue;
            }

            let rep_pos = q_stockpiles
                .get(receiver)
                .map(|(_, t, _, _, _)| t.translation.truncate())
                .unwrap_or(Vec2::ZERO);

            desired_requests.insert(
                (receiver, resource_type),
                (group.owner_familiar, donor_cells, transfer_count, rep_pos),
            );
        }
    }

    // 既存リクエストの upsert / cleanup
    let mut seen = std::collections::HashSet::new();
    for (req_entity, req, workers_opt) in q_existing_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::ConsolidateStockpile) {
            continue;
        }
        let key = (req.anchor, req.resource_type);
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(&mut commands, req_entity, workers, &mut seen, key)
        {
            continue;
        }

        if let Some((issued_by, donor_cells, transfer_count, pos)) = desired_requests.get(&key) {
            commands.entity(req_entity).try_insert((
                Transform::from_xyz(pos.x, pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*transfer_count as u32),
                Priority(0),
                TransportRequest {
                    kind: TransportRequestKind::ConsolidateStockpile,
                    anchor: key.0,
                    resource_type: key.1,
                    issued_by: *issued_by,
                    priority: TransportPriority::Low,
                    stockpile_group: donor_cells.clone(),
                },
                TransportDemand {
                    desired_slots: *transfer_count as u32,
                    inflight: 0,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
        } else if workers == 0 {
            super::upsert::disable_request(&mut commands, req_entity);
        }
    }

    // 新規リクエスト生成
    for (key, (issued_by, donor_cells, transfer_count, pos)) in desired_requests {
        if seen.contains(&key) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::ConsolidateStockpile"),
            Transform::from_xyz(pos.x, pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(transfer_count as u32),
            Priority(0),
            TransportRequest {
                kind: TransportRequestKind::ConsolidateStockpile,
                anchor: key.0,
                resource_type: key.1,
                issued_by,
                priority: TransportPriority::Low,
                stockpile_group: donor_cells,
            },
            TransportDemand {
                desired_slots: transfer_count as u32,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}
