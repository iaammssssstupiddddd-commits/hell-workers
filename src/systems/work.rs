use bevy::prelude::*;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::entities::familiar::{ActiveCommand, FamiliarCommand, UnderCommand};
use crate::assets::GameAssets;
use crate::systems::command::TaskArea;
use crate::systems::logistics::{ResourceItem, ClaimedBy, Stockpile, Inventory, InStockpile};
use crate::systems::jobs::{Designation, WorkType, Tree, Rock, IssuedBy};
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
    mut q_familiars: Query<(Entity, &Transform, &mut ActiveCommand)>,
    mut q_souls: Query<(Entity, &Transform, &DamnedSoul, &mut AssignedTask, &mut Destination, &mut Path, &mut Inventory)>,
    q_designations: Query<(Entity, &Transform, &Designation, &IssuedBy), (Without<ClaimedBy>, Without<InStockpile>)>,
    q_stockpiles: Query<(Entity, &Transform), With<Stockpile>>,
) {
    for (fam_entity, fam_transform, mut active_command) in q_familiars.iter_mut() {
        let fam_pos = fam_transform.translation.truncate();
        
        // 使役枠の空きを確認 (最大2名)
        let current_count = active_command.assigned_souls.len();
        if current_count >= 2 { continue; }
        let slots_available = 2 - current_count;

        // この使い魔が発行した未割当タスクを探す
        let mut my_designations: Vec<_> = q_designations.iter()
            .filter(|(_, _, _, issued_by)| issued_by.0 == fam_entity)
            .map(|(e, t, d, _)| (e, t.translation.truncate(), d))
            .collect();

        // 近い順にソートしておくと効率的かも
        my_designations.sort_by(|(_, p1, _), (_, p2, _)| {
            let d1 = p1.distance_squared(fam_pos);
            let d2 = p2.distance_squared(fam_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut assigned_this_tick = 0;
        for (des_entity, des_pos, designation) in my_designations.iter() {
            if assigned_this_tick >= slots_available { break; }

            // 近くのフリーな魂を探す
            let best_soul = q_souls.iter()
                .filter(|(_, _, soul, current_task, _, _, _)| {
                    matches!(*current_task, AssignedTask::None) && 
                    soul.motivation >= MOTIVATION_THRESHOLD && 
                    soul.fatigue < FATIGUE_THRESHOLD
                })
                .min_by(|(_, t1, _, _, _, _, _), (_, t2, _, _, _, _, _)| {
                    let d1 = t1.translation.truncate().distance_squared(*des_pos);
                    let d2 = t2.translation.truncate().distance_squared(*des_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, _, _, _, _, _, _)| e);

            if let Some(soul_entity) = best_soul {
                match designation.work_type {
                    WorkType::Chop | WorkType::Mine => {
                        if let Ok((mut soul_task, mut dest, mut path)) = q_souls.get_mut(soul_entity).map(|(_, _, _, t, d, p, _)| (t, d, p)) {
                            *soul_task = AssignedTask::Gather {
                                target: *des_entity,
                                phase: GatherPhase::GoingToResource,
                            };
                            dest.0 = *des_pos;
                            path.waypoints.clear();
                            
                            // リンクの作成
                            commands.entity(*des_entity).insert(ClaimedBy(soul_entity));
                            commands.entity(soul_entity).insert(UnderCommand(fam_entity));
                            active_command.assigned_souls.push(soul_entity);
                            
                            assigned_this_tick += 1;
                            info!("DELEGATION: Soul {:?} assigned to GATHER target {:?} by Familiar {:?}", soul_entity, des_entity, fam_entity);
                        }
                    }
                    WorkType::Haul => {
                        let best_stockpile = q_stockpiles.iter()
                            .min_by(|(_, t1), (_, t2)| {
                                let d1 = t1.translation.truncate().distance_squared(*des_pos);
                                let d2 = t2.translation.truncate().distance_squared(*des_pos);
                                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(e, _)| e);

                        if let Some(stock_entity) = best_stockpile {
                            if let Ok((mut soul_task, mut dest, mut path)) = q_souls.get_mut(soul_entity).map(|(_, _, _, t, d, p, _)| (t, d, p)) {
                                *soul_task = AssignedTask::Haul {
                                    item: *des_entity,
                                    stockpile: stock_entity,
                                    phase: HaulPhase::GoingToItem,
                                };
                                dest.0 = *des_pos;
                                path.waypoints.clear();
                                
                                // リンクの作成
                                commands.entity(*des_entity).insert(ClaimedBy(soul_entity));
                                commands.entity(soul_entity).insert(UnderCommand(fam_entity));
                                active_command.assigned_souls.push(soul_entity);
                                
                                assigned_this_tick += 1;
                                info!("DELEGATION: Soul {:?} assigned HAUL item {:?} by Familiar {:?}", soul_entity, des_entity, fam_entity);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// タスクが完了した魂を使役から解放する
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_familiars: Query<&mut ActiveCommand>,
    q_souls: Query<(Entity, &AssignedTask, &UnderCommand)>,
) {
    for (soul_entity, task, under_command) in q_souls.iter() {
        if matches!(task, AssignedTask::None) {
            let fam_entity = under_command.0;
            if let Ok(mut active_command) = q_familiars.get_mut(fam_entity) {
                // 使い魔のリストから削除
                active_command.assigned_souls.retain(|&e| e != soul_entity);
            }
            // コンポーネントを削除して解放
            commands.entity(soul_entity).remove::<UnderCommand>();
            info!("RELEASE: Soul {:?} released from Familiar {:?}", soul_entity, fam_entity);
        }
    }
}

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: Query<(Entity, &Transform, &mut DamnedSoul, &mut AssignedTask, &mut Destination, &mut Path, &mut Inventory)>,
    q_targets: Query<(&Transform, Option<&Tree>, Option<&Rock>, Option<&ResourceItem>)>,
    q_stockpiles: Query<&Transform, With<Stockpile>>,
    game_assets: Res<GameAssets>,
) {
    for (soul_entity, soul_transform, mut soul, mut task, mut dest, mut path, mut inventory) in q_souls.iter_mut() {
        if !matches!(*task, AssignedTask::None) {
            // debug!("TASK_EXEC: Soul {:?} is on task {:?}", soul_entity, *task);
        }
        match *task {
            AssignedTask::Gather { target, phase } => {
                handle_gather_task(soul_entity, soul_transform, &mut soul, &mut task, &mut dest, &mut path, target, phase, &q_targets, &mut commands, &game_assets);
            }
            AssignedTask::Haul { item, stockpile, phase } => {
                handle_haul_task(soul_entity, soul_transform, &mut soul, &mut task, &mut dest, &mut path, &mut inventory, item, stockpile, phase, &q_targets, &q_stockpiles, &mut commands);
            }
            AssignedTask::None => {}
        }
    }
}

fn handle_gather_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    target: Entity,
    phase: GatherPhase,
    q_targets: &Query<(&Transform, Option<&Tree>, Option<&Rock>, Option<&ResourceItem>)>,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
) {
    let soul_pos = soul_transform.translation.truncate();
    match phase {
        GatherPhase::GoingToResource => {
            if let Ok((res_transform, _, _, _)) = q_targets.get(target) {
                let res_pos = res_transform.translation.truncate();
                if dest.0.distance_squared(res_pos) > 2.0 {
                    dest.0 = res_pos;
                    path.waypoints.clear();
                }

                let dist = soul_pos.distance(res_pos);
                if dist < TILE_SIZE * 1.2 {
                    *task = AssignedTask::Gather { target, phase: GatherPhase::Collecting };
                    path.waypoints.clear();
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        GatherPhase::Collecting => {
            if let Ok((res_transform, tree, rock, _)) = q_targets.get(target) {
                let pos = res_transform.translation;
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
                } else if rock.is_some() {
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
            *task = AssignedTask::Gather { target, phase: GatherPhase::Done };
            soul.fatigue = (soul.fatigue + 0.1).min(1.0);
        }
        GatherPhase::Done => {
            *task = AssignedTask::None;
            path.waypoints.clear();
        }
    }
}

fn handle_haul_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    inventory: &mut Inventory,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    q_targets: &Query<(&Transform, Option<&Tree>, Option<&Rock>, Option<&ResourceItem>)>,
    q_stockpiles: &Query<&Transform, With<Stockpile>>,
    commands: &mut Commands,
) {
    let soul_pos = soul_transform.translation.truncate();
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _)) = q_targets.get(item) {
                let res_pos = res_transform.translation.truncate();
                if dest.0.distance_squared(res_pos) > 2.0 {
                    dest.0 = res_pos;
                    path.waypoints.clear();
                }
                
                if soul_pos.distance(res_pos) < TILE_SIZE * 1.2 {
                    inventory.0 = Some(item);
                    commands.entity(item).insert(Visibility::Hidden);
                    *task = AssignedTask::Haul { item, stockpile, phase: HaulPhase::GoingToStockpile };
                    path.waypoints.clear();
                    info!("HAUL: Soul {:?} picked up item", soul_entity);
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok(stock_transform) = q_stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                if dest.0.distance_squared(stock_pos) > 2.0 {
                    dest.0 = stock_pos;
                    path.waypoints.clear();
                }
                
                if soul_pos.distance(stock_pos) < TILE_SIZE * 1.2 {
                    *task = AssignedTask::Haul { item, stockpile, phase: HaulPhase::Dropping };
                    path.waypoints.clear();
                }
            } else {
                warn!("HAUL: Soul {:?} stockpile {:?} not found", soul_entity, stockpile);
                if let Some(item_entity) = inventory.0.take() {
                    commands.entity(item_entity).insert(Visibility::Visible);
                    commands.entity(item_entity).remove::<ClaimedBy>();
                }
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

pub fn task_area_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(&Transform, &Stockpile)>,
    q_items_in_stock: Query<&Transform, With<InStockpile>>,
    q_resources: Query<(Entity, &Transform), (With<ResourceItem>, Without<InStockpile>, Without<Designation>)>,
) {
    for (fam_entity, active_command, task_area) in q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        for (stock_transform, stockpile) in q_stockpiles.iter() {
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                continue;
            }

            let current_count = q_items_in_stock.iter()
                .filter(|t| WorldMap::world_to_grid(t.translation.truncate()) == WorldMap::world_to_grid(stock_pos))
                .count();

            if current_count >= stockpile.capacity {
                continue;
            }

            let nearest_resource = q_resources.iter()
                .filter(|(_, t)| t.translation.truncate().distance_squared(stock_pos) < (TILE_SIZE * 15.0).powi(2))
                .min_by(|(_, t1), (_, t2)| {
                    let d1 = t1.translation.truncate().distance_squared(stock_pos);
                    let d2 = t2.translation.truncate().distance_squared(stock_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some((item_entity, _)) = nearest_resource {
                commands.entity(item_entity).insert((
                    Designation { work_type: WorkType::Haul },
                    IssuedBy(fam_entity),
                ));
                debug!("AUTO_HAUL: Designated item {:?} for stockpile (IssuedBy: {:?})", item_entity, fam_entity);
            }
        }
    }
}
