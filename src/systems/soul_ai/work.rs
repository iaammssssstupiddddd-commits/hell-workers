//! 作業管理モジュール
//!
//! 魂へのタスク解除や自動運搬ロジックを管理します。

use crate::constants::*;
use crate::entities::damned_soul::Path;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::relationships::{Holding, TaskWorkers, WorkingOn};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::jobs::{Designation, DesignationCreatedEvent, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::{ResourceItem, Stockpile};
use crate::systems::soul_ai::execution::AssignedTask;
use crate::systems::spatial::ResourceSpatialGrid;
use crate::world::map::WorldMap;
use bevy::prelude::*;

// ============================================================
// 自動運搬関連
// ============================================================

/// 実行頻度を制御するためのカウンター
#[derive(Resource, Default)]
pub struct AutoHaulCounter;

// ============================================================
// システム実装
// ============================================================

/// 使い魔が Idle コマンドの場合、または使い魔が存在しない場合に部下をリリースする
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &UnderCommand,
        &mut AssignedTask,
        &mut Path,
        Option<&Holding>,
    )>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    q_familiars: Query<&ActiveCommand, With<Familiar>>,
    mut haul_cache: ResMut<HaulReservationCache>,
) {
    for (soul_entity, transform, under_command, mut task, mut path, holding_opt) in
        q_souls.iter_mut()
    {
        let should_release = match q_familiars.get(under_command.0) {
            Ok(active_cmd) => matches!(active_cmd.command, FamiliarCommand::Idle),
            Err(_) => true,
        };

        if should_release {
            info!(
                "RELEASE: Soul {:?} released from Familiar {:?}",
                soul_entity, under_command.0
            );

            unassign_task(
                &mut commands,
                soul_entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                holding_opt,
                &q_designations,
                &mut *haul_cache,
            );

            commands.entity(soul_entity).remove::<UnderCommand>();
        }
    }
}

/// 魂からタスクの割り当てを解除し、スロットを解放する。
pub fn unassign_task(
    commands: &mut Commands,
    soul_entity: Entity,
    drop_pos: Vec2,
    task: &mut AssignedTask,
    path: &mut Path,
    holding: Option<&Holding>,
    q_designations: &Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    haul_cache: &mut HaulReservationCache,
) {
    if let AssignedTask::Haul { stockpile, .. } = *task {
        haul_cache.release(stockpile);
    }

    if let Some(Holding(item_entity)) = holding {
        let item_entity = *item_entity;
        let grid = WorldMap::world_to_grid(drop_pos);
        let snapped_pos = WorldMap::grid_to_world(grid.0, grid.1);

        commands.entity(item_entity).insert((
            Visibility::Visible,
            Transform::from_xyz(snapped_pos.x, snapped_pos.y, 0.6),
        ));
        commands.entity(soul_entity).remove::<Holding>();

        info!(
            "UNASSIGN: Soul released item {:?} at {:?} (snapped to {:?})",
            item_entity, drop_pos, snapped_pos
        );
    }

    let target_entity = match *task {
        AssignedTask::Gather { target, .. } => Some(target),
        AssignedTask::Haul { item, .. } => Some(item),
        AssignedTask::None => None,
    };

    if let Some(target) = target_entity {
        if let Ok((_, _, _, _, _, _)) = q_designations.get(target) {
            commands.entity(target).remove::<Designation>();
            commands.entity(target).remove::<TaskSlots>();
            commands.entity(target).remove::<IssuedBy>();
        }
    }

    commands.entity(soul_entity).remove::<WorkingOn>();

    *task = AssignedTask::None;
    path.waypoints.clear();
}

/// 指揮エリア内での自動運搬タスク生成システム
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    _counter: ResMut<AutoHaulCounter>,
    resource_grid: Res<ResourceSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        &Transform,
        &Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_resources: Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
            Without<TaskWorkers>,
        ),
    >,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
) {
    let mut already_assigned = std::collections::HashSet::new();

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        for (stock_transform, stockpile, stored_items_opt) in q_stockpiles.iter() {
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                continue;
            }

            let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
            if current_count >= stockpile.capacity {
                continue;
            }

            let search_radius = TILE_SIZE * 15.0;
            let nearby_resources = resource_grid.get_nearby_in_radius(stock_pos, search_radius);

            let nearest_resource = nearby_resources
                .iter()
                .filter(|&&entity| !already_assigned.contains(&entity))
                .filter_map(|&entity| {
                    let Ok((_, transform, visibility, res_item)) = q_resources.get(entity) else {
                        return None;
                    };
                    if *visibility == Visibility::Hidden {
                        return None;
                    }

                    if let Some(target_type) = stockpile.resource_type {
                        if res_item.0 != target_type {
                            return None;
                        }
                    }

                    let dist_sq = transform.translation.truncate().distance_squared(stock_pos);
                    if dist_sq < search_radius * search_radius {
                        Some((entity, dist_sq))
                    } else {
                        None
                    }
                })
                .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(entity, _)| entity);

            if let Some(item_entity) = nearest_resource {
                already_assigned.insert(item_entity);
                commands.entity(item_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                ));
                ev_created.write(DesignationCreatedEvent {
                    entity: item_entity,
                    work_type: WorkType::Haul,
                    issued_by: Some(fam_entity),
                    priority: 0,
                });
            }
        }
    }
}
