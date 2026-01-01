use bevy::prelude::*;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};
use crate::systems::command::TaskArea;
use crate::systems::logistics::{ResourceItem, ClaimedBy, Stockpile, Inventory, InStockpile};

/// 人間に割り当てられたタスク
#[derive(Component, Clone, Debug)]
pub enum AssignedTask {
    None,
    /// リソースを収集する（簡略版）
    Gather {
        target: Entity,
        phase: GatherPhase,
    },
    /// リソースを運搬する
    Haul {
        item: Entity,
        stockpile: Entity,
        phase: HaulPhase,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaulPhase {
    GoingToItem,
    GoingToStockpile,
    Dropping,
}

impl Default for AssignedTask {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatherPhase {
    GoingToResource,
    Collecting,
    Done,
}

pub fn task_delegation_system(
    mut commands: Commands,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand, Option<&TaskArea>)>,
    mut q_souls: Query<(Entity, &Transform, &DamnedSoul, &mut AssignedTask)>,
    q_resources: Query<(Entity, &Transform), (With<ResourceItem>, Without<ClaimedBy>, Without<InStockpile>)>,
    q_stockpiles: Query<(Entity, &Transform), With<Stockpile>>,
) {

    for (fam_transform, familiar, command, task_area) in q_familiars.iter() {
        if matches!(command.command, FamiliarCommand::Idle) {
            continue;
        }

        // 指示を中心に（指示が届く範囲）
        let command_center = fam_transform.translation.truncate();
        let command_radius = familiar.command_radius;

        for (soul_entity, soul_transform, soul, mut current_task) in q_souls.iter_mut() {
            if !matches!(*current_task, AssignedTask::None) {
                continue;
            }

            let soul_pos = soul_transform.translation.truncate();
            let dist_to_fam = soul_pos.distance(command_center);

            if dist_to_fam > command_radius {
                continue;
            }

            if soul.motivation < 0.1 {
                continue;
            }

            // 収集・運搬コマンドの処理
            if matches!(command.command, FamiliarCommand::GatherResources) || matches!(command.command, FamiliarCommand::Patrol) {
                
                // 1. エリア内にストックパイルがあるか探す
                let mut target_stockpile: Option<Entity> = None;
                if let Some(area) = task_area {
                    for (stock_entity, stock_transform) in q_stockpiles.iter() {
                        let stock_pos = stock_transform.translation.truncate();
                        if stock_pos.distance(area.center) < area.radius {
                            target_stockpile = Some(stock_entity);
                            break; 
                        }
                    }
                }

                if let Some(stock_entity) = target_stockpile {
                    // 2. 運搬モード：最も近いリソースを探す
                    let mut closest: Option<(Entity, f32)> = None;
                    for (res_entity, res_transform) in q_resources.iter() {
                        let res_pos = res_transform.translation.truncate();
                        let dist_to_soul = soul_pos.distance(res_pos);
                        if closest.is_none() || dist_to_soul < closest.unwrap().1 {
                            closest = Some((res_entity, dist_to_soul));
                        }
                    }

                    if let Some((res_entity, _)) = closest {
                        *current_task = AssignedTask::Haul {
                            item: res_entity,
                            stockpile: stock_entity,
                            phase: HaulPhase::GoingToItem,
                        };
                        commands.entity(res_entity).insert(ClaimedBy(soul_entity));
                        debug!("DELEGATION: Soul {:?} assigned HAUL to {:?}", soul_entity, stock_entity);
                        continue;
                    }
                }

                // 3. 収集モード：エリア内のリソースを探す
                let mut closest: Option<(Entity, f32)> = None;
                let (target_center, target_radius) = if let Some(area) = task_area {
                    (area.center, area.radius)
                } else {
                    (command_center, command_radius * 1.5)
                };

                for (res_entity, res_transform) in q_resources.iter() {
                    let res_pos = res_transform.translation.truncate();
                    if res_pos.distance(target_center) < target_radius {
                        let dist_to_soul = soul_pos.distance(res_pos);
                        if closest.is_none() || dist_to_soul < closest.unwrap().1 {
                            closest = Some((res_entity, dist_to_soul));
                        }
                    }
                }

                if let Some((res_entity, dist)) = closest {
                    *current_task = AssignedTask::Gather {
                        target: res_entity,
                        phase: GatherPhase::GoingToResource,
                    };
                    commands.entity(res_entity).insert(ClaimedBy(soul_entity));
                    debug!("DELEGATION: Soul {:?} assigned GATHER at dist={:.1}", soul_entity, dist);
                }
            }
        }
    }
}

/// タスクを実行するシステム
pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: Query<(Entity, &Transform, &mut DamnedSoul, &mut AssignedTask, &mut Destination, &mut Path, &mut Inventory)>,
    q_resources: Query<&Transform, With<ResourceItem>>,
    q_stockpiles: Query<&Transform, With<Stockpile>>,
) {
    for (soul_entity, soul_transform, mut soul, mut task, mut dest, mut path, mut inventory) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();

        match *task {
            AssignedTask::Gather { target, phase } => {
                match phase {
                    GatherPhase::GoingToResource => {
                        if let Ok(res_transform) = q_resources.get(target) {
                            let res_pos = res_transform.translation.truncate();
                            let dist_to_res = soul_pos.distance(res_pos);

                            let dist_to_current_dest = dest.0.distance(res_pos);
                            if path.waypoints.is_empty() || dist_to_current_dest > 1.0 {
                                dest.0 = res_pos;
                                debug!("TASK_EXEC: Soul {:?} heading to target", soul_entity);
                            }

                            if dist_to_res < TILE_SIZE * 0.8 {
                                *task = AssignedTask::Gather {
                                    target,
                                    phase: GatherPhase::Collecting,
                                };
                                debug!("TASK_EXEC: Soul {:?} arrived at resource", soul_entity);
                            }
                        } else {
                            *task = AssignedTask::None;
                            path.waypoints.clear();
                        }
                    }
                    GatherPhase::Collecting => {
                        if let Ok(_) = q_resources.get(target) {
                            commands.entity(target).despawn();
                            info!("TASK_EXEC: Soul {:?} collected resource", soul_entity);
                        }
                        *task = AssignedTask::Gather {
                            target,
                            phase: GatherPhase::Done,
                        };
                        soul.fatigue = (soul.fatigue + 0.1).min(1.0);
                    }
                    GatherPhase::Done => {
                        *task = AssignedTask::None;
                        path.waypoints.clear();
                    }
                }
            }
            AssignedTask::Haul { item, stockpile, phase } => {
                match phase {
                    HaulPhase::GoingToItem => {
                        if let Ok(res_transform) = q_resources.get(item) {
                            let res_pos = res_transform.translation.truncate();
                            if dest.0.distance(res_pos) > 1.0 {
                                dest.0 = res_pos;
                            }
                            if soul_pos.distance(res_pos) < TILE_SIZE * 0.7 {
                                inventory.0 = Some(item);
                                commands.entity(item).insert(Visibility::Hidden);
                                *task = AssignedTask::Haul {
                                    item,
                                    stockpile,
                                    phase: HaulPhase::GoingToStockpile,
                                };
                                path.waypoints.clear();
                                info!("TASK_EXEC: Soul {:?} picked up item", soul_entity);
                            }
                        } else {
                            *task = AssignedTask::None;
                            path.waypoints.clear();
                        }
                    }
                    HaulPhase::GoingToStockpile => {
                        if let Ok(stock_transform) = q_stockpiles.get(stockpile) {
                            let stock_pos = stock_transform.translation.truncate();
                            if dest.0.distance(stock_pos) > 1.0 {
                                dest.0 = stock_pos;
                            }
                            if soul_pos.distance(stock_pos) < TILE_SIZE * 0.7 {
                                *task = AssignedTask::Haul {
                                    item,
                                    stockpile,
                                    phase: HaulPhase::Dropping,
                                };
                                debug!("TASK_EXEC: Soul {:?} arrived at stockpile", soul_entity);
                            }
                        } else {
                            *task = AssignedTask::None;
                            path.waypoints.clear();
                        }
                    }
                    HaulPhase::Dropping => {
                        if let Ok(stock_transform) = q_stockpiles.get(stockpile) {
                            let stock_pos = stock_transform.translation.truncate();
                            if let Some(item_entity) = inventory.0.take() {
                                commands.entity(item_entity).insert((
                                    Visibility::Visible,
                                    Transform::from_xyz(stock_pos.x, stock_pos.y, 0.6),
                                    InStockpile,
                                ));
                                commands.entity(item_entity).remove::<ClaimedBy>();
                                info!("TASK_EXEC: Soul {:?} dropped item at stockpile", soul_entity);
                            }
                        }
                        *task = AssignedTask::None;
                        path.waypoints.clear();
                        soul.fatigue = (soul.fatigue + 0.05).min(1.0);
                    }
                }
            }
            AssignedTask::None => {}
        }
    }
}
