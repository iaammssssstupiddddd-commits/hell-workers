use bevy::prelude::*;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::assets::GameAssets;
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};
use crate::systems::command::TaskArea;
use crate::systems::logistics::{ResourceItem, ClaimedBy, Stockpile, Inventory, InStockpile};
use crate::systems::jobs::{Designation, WorkType, Tree, Rock};
use crate::world::map::WorldMap;

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
    q_designations: Query<(Entity, &Transform, &Designation), Without<ClaimedBy>>,
    q_stockpiles: Query<(Entity, &Transform), With<Stockpile>>,
    _q_resources: Query<Entity, With<ResourceItem>>,
) {
    for (fam_transform, familiar, command, task_area) in q_familiars.iter() {
        if matches!(command.command, FamiliarCommand::Idle) {
            continue;
        }

        let fam_pos = fam_transform.translation.truncate();
        let command_radius = familiar.command_radius;

        // 1. 周囲の指示（Designation）を探す
        for (des_entity, des_transform, designation) in q_designations.iter() {
            let des_pos = des_transform.translation.truncate();
            
            // 指示エリアの判定
            if let Some(area) = task_area {
                let mut valid = area.contains(des_pos);
                
                // 運搬タスクの場合、運び先の倉庫がエリア内なら許可（遠くの資源も回収させるため）
                if !valid && matches!(designation.work_type, WorkType::Haul) {
                    let mut nearest_stock_pos: Option<Vec2> = None;
                    let mut min_dist = f32::MAX;
                    for (_, s_transform) in q_stockpiles.iter() {
                        let s_pos = s_transform.translation.truncate();
                        let dist = des_pos.distance(s_pos);
                        if dist < min_dist {
                            min_dist = dist;
                            nearest_stock_pos = Some(s_pos);
                        }
                    }
                    if let Some(s_pos) = nearest_stock_pos {
                        if area.contains(s_pos) {
                            valid = true;
                        }
                    }
                }

                if !valid {
                    continue;
                }
            } else {
                // エリア指定がない場合は使い魔の指揮範囲内か？
                if des_pos.distance(fam_pos) > command_radius {
                    continue;
                }
            }

            // 2. この仕事に割り当てる「暇な魂」を探す
            let mut best_soul: Option<(Entity, f32)> = None;
            for (soul_entity, soul_transform, soul, current_task) in q_souls.iter() {
                if !matches!(*current_task, AssignedTask::None) || soul.motivation < 0.1 {
                    continue;
                }
                let soul_pos = soul_transform.translation.truncate();
                let dist = soul_pos.distance(des_pos);
                if best_soul.is_none() || dist < best_soul.unwrap().1 {
                    best_soul = Some((soul_entity, dist));
                }
            }

            if let Some((soul_entity, _)) = best_soul {
                // タスク割り当て
                match designation.work_type {
                    WorkType::Chop | WorkType::Mine => {
                        if let Ok(mut soul_task) = q_souls.get_mut(soul_entity).map(|(_, _, _, t)| t) {
                            *soul_task = AssignedTask::Gather {
                                target: des_entity,
                                phase: GatherPhase::GoingToResource,
                            };
                            commands.entity(des_entity).insert(ClaimedBy(soul_entity));
                            debug!("DELEGATION: Soul {:?} assigned to designation {:?}", soul_entity, des_entity);
                            break; // 一つの指示に一人の魂（簡略化のため）
                        }
                    }
                    WorkType::Haul => {
                        // 運搬の場合は最寄りのストックパイルも探す
                        let mut best_stockpile: Option<Entity> = None;
                        let mut min_stock_dist = f32::MAX;
                        for (stock_entity, stock_transform) in q_stockpiles.iter() {
                            let dist = stock_transform.translation.truncate().distance(des_pos);
                            if dist < min_stock_dist {
                                min_stock_dist = dist;
                                best_stockpile = Some(stock_entity);
                            }
                        }

                        if let Some(stock_entity) = best_stockpile {
                            if let Ok(mut soul_task) = q_souls.get_mut(soul_entity).map(|(_, _, _, t)| t) {
                                *soul_task = AssignedTask::Haul {
                                    item: des_entity,
                                    stockpile: stock_entity,
                                    phase: HaulPhase::GoingToItem,
                                };
                                commands.entity(des_entity).insert(ClaimedBy(soul_entity));
                                debug!("DELEGATION: Soul {:?} assigned HAUL designation {:?}", soul_entity, des_entity);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// タスクを実行するシステム
pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: Query<(Entity, &Transform, &mut DamnedSoul, &mut AssignedTask, &mut Destination, &mut Path, &mut Inventory)>,
    q_targets: Query<(&Transform, Option<&Tree>, Option<&Rock>, Option<&ResourceItem>)>,
    q_stockpiles: Query<&Transform, With<Stockpile>>,
    game_assets: Res<GameAssets>,
) {
    for (soul_entity, soul_transform, mut soul, mut task, mut dest, mut path, mut inventory) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();

        match *task {
            AssignedTask::Gather { target, phase } => {
                match phase {
                    GatherPhase::GoingToResource => {
                        if let Ok((res_transform, _, _, _)) = q_targets.get(target) {
                            let res_pos = res_transform.translation.truncate();
                            let dist_to_res = soul_pos.distance(res_pos);

                            let dist_to_current_dest = dest.0.distance(res_pos);
                            if path.waypoints.is_empty() || dist_to_current_dest > 1.0 {
                                dest.0 = res_pos;
                            }

                            if dist_to_res < TILE_SIZE * 0.8 {
                                *task = AssignedTask::Gather {
                                    target,
                                    phase: GatherPhase::Collecting,
                                };
                            }
                        } else {
                            *task = AssignedTask::None;
                            path.waypoints.clear();
                        }
                    }
                    GatherPhase::Collecting => {
                        if let Ok((res_transform, tree, rock, _item)) = q_targets.get(target) {
                            let pos = res_transform.translation;
                            
                            // 木を伐採した場合は木材を落とす
                            if tree.is_some() {
                                commands.spawn((
                                    ResourceItem(crate::systems::logistics::ResourceType::Wood),
                                    Sprite {
                                        image: game_assets.wood.clone(),
                                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                        color: Color::srgb(0.5, 0.35, 0.05),
                                        ..default()
                                    },
                                    Transform::from_translation(pos),
                                ));
                                info!("TASK_EXEC: Soul {:?} chopped a tree", soul_entity);
                            } 
                            // 岩を掘った場合は石材（仮で木材）を落とす
                            else if rock.is_some() {
                                // 本来はResourceType::Stoneだが現状Woodしかないので仮
                                commands.spawn((
                                    ResourceItem(crate::systems::logistics::ResourceType::Wood),
                                    Sprite {
                                        image: game_assets.stone.clone(),
                                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                        ..default()
                                    },
                                    Transform::from_translation(pos),
                                ));
                                info!("TASK_EXEC: Soul {:?} mined a rock", soul_entity);
                            }
                            
                            commands.entity(target).despawn();
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
                        if let Ok((res_transform, _, _, _)) = q_targets.get(item) {
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

/// タスクエリア内に倉庫がある場合、自動的に運搬指示を出すシステム
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(&ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(&Transform, &Stockpile)>,
    q_items_in_stock: Query<&Transform, With<InStockpile>>,
    q_resources: Query<(Entity, &Transform), (With<ResourceItem>, Without<InStockpile>, Without<Designation>)>,
) {
    for (active_command, task_area) in q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        // 1. エリア内の倉庫を探す
        for (stock_transform, stockpile) in q_stockpiles.iter() {
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                continue;
            }

            // 2. 倉庫の空きを確認 (そのタイルにあるアイテム数)
            let current_count = q_items_in_stock.iter()
                .filter(|t| WorldMap::world_to_grid(t.translation.truncate()) == WorldMap::world_to_grid(stock_pos))
                .count();

            if current_count >= stockpile.capacity {
                continue;
            }

            // 3. 付近の未指定リソースを探してHaul指示を出す
            let mut nearest_resource: Option<(Entity, f32)> = None;
            for (item_entity, item_transform) in q_resources.iter() {
                let item_pos = item_transform.translation.truncate();
                let dist = item_pos.distance(stock_pos);
                
                // 倉庫から一定範囲内（例：15タイル）のものを対象
                if dist < TILE_SIZE * 15.0 {
                    if nearest_resource.is_none() || dist < nearest_resource.unwrap().1 {
                        nearest_resource = Some((item_entity, dist));
                    }
                }
            }

            if let Some((item_entity, _)) = nearest_resource {
                commands.entity(item_entity).insert(Designation { work_type: WorkType::Haul });
                debug!("AUTO_HAUL: Designated item {:?} for stockpile", item_entity);
            }
        }
    }
}
