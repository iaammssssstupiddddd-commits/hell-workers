//! タスク割り当てモジュール
//!
//! ワーカーへのタスク割り当てロジックを提供します。

use crate::entities::damned_soul::IdleBehavior;
use crate::relationships::CommandedBy;
use crate::events::{OnSoulRecruited, ResourceReservationOp, TaskAssignmentRequest};
use crate::systems::command::TaskArea;
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::task_execution::types::{
    AssignedTask, BuildPhase, GatherPhase, GatherWaterPhase, HaulPhase, HaulToBpPhase,
};

use bevy::prelude::*;
use crate::systems::familiar_ai::FamiliarSoulQuery;
use std::collections::HashMap;

/// Thinkフェーズ内の予約増分を追跡する
#[derive(Default)]
pub struct ReservationShadow {
    destination: HashMap<Entity, usize>,
    mixer_destination: HashMap<(Entity, ResourceType), usize>,
    source: HashMap<Entity, usize>,
}

impl ReservationShadow {
    pub fn destination_reserved(&self, target: Entity) -> usize {
        self.destination.get(&target).cloned().unwrap_or(0)
    }

    pub fn mixer_reserved(&self, target: Entity, resource_type: ResourceType) -> usize {
        self.mixer_destination
            .get(&(target, resource_type))
            .cloned()
            .unwrap_or(0)
    }

    pub fn source_reserved(&self, source: Entity) -> usize {
        self.source.get(&source).cloned().unwrap_or(0)
    }

    pub fn apply_reserve_ops(&mut self, ops: &[ResourceReservationOp]) {
        for op in ops {
            match *op {
                ResourceReservationOp::ReserveDestination { target } => {
                    *self.destination.entry(target).or_insert(0) += 1;
                }
                ResourceReservationOp::ReserveMixerDestination { target, resource_type } => {
                    *self.mixer_destination.entry((target, resource_type)).or_insert(0) += 1;
                }
                ResourceReservationOp::ReserveSource { source, amount } => {
                    *self.source.entry(source).or_insert(0) += amount;
                }
                _ => {}
            }
        }
    }
}

/// 予約チェックヘルパー: 既に予約済み人数がスロット上限に達しているか確認
fn can_reserve_source(
    task_entity: Entity,
    // resource_cache removed
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    // 現在の予約数（実行中 + 割り当て済み移動中）
    let current_reserved = queries.resource_cache.get_source_reservation(task_entity)
        + shadow.source_reserved(task_entity);

    // TaskSlotsコンポーネントがあればそのmax値、なければデフォルト1（排他）
    let max_slots = if let Ok(slots) = queries.task_slots.get(task_entity) {
        slots.max as usize
    } else {
        1
    };

    current_reserved < max_slots
}

/// ワーカーにタスク割り当てのための共通セットアップを行う
pub fn prepare_worker_for_task(
    commands: &mut Commands,
    worker_entity: Entity,
    fam_entity: Entity,
    task_entity: Entity,
    already_commanded: bool,
) {
    if !already_commanded {
        commands.trigger(OnSoulRecruited {
            entity: worker_entity,
            familiar_entity: fam_entity,
        });
    }
    commands.entity(worker_entity).insert((
        CommandedBy(fam_entity),
        crate::relationships::WorkingOn(task_entity),
    ));
    commands
        .entity(task_entity)
        .insert(crate::systems::jobs::IssuedBy(fam_entity));
}

/// ワーカーにタスクを割り当てる
#[allow(clippy::too_many_arguments)]
pub fn assign_task_to_worker(
    fam_entity: Entity,
    task_entity: Entity,
    worker_entity: Entity,
    fatigue_threshold: f32,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    q_souls: &mut FamiliarSoulQuery,
    task_area_opt: Option<&TaskArea>,
    shadow: &mut ReservationShadow,
) -> bool {
    let Ok((_, _, soul, _assigned_task, _dest, _path, idle, _, uc_opt, _participating_opt)) =
        q_souls.get_mut(worker_entity)
    else {
        warn!("ASSIGN: Worker {:?} not found in query", worker_entity);
        return false;
    };

    if idle.behavior == IdleBehavior::ExhaustedGathering {
        debug!("ASSIGN: Worker {:?} is exhausted gathering", worker_entity);
        return false;
    }

    if soul.fatigue >= fatigue_threshold {
        debug!("ASSIGN: Worker {:?} is too fatigued ({:.2} >= {:.2})", worker_entity, soul.fatigue, fatigue_threshold);
        return false;
    }

    // タスクが存在するか最終確認
    let (task_pos, work_type) =
        if let Ok((_, transform, designation, _, _, _, _, _)) = queries.designations.get(task_entity) {
            (transform.translation.truncate(), designation.work_type)
        } else {
            debug!("ASSIGN: Task designation {:?} disappeared", task_entity);
            return false;
        };

    match work_type {
        WorkType::Chop | WorkType::Mine => {
            if !can_reserve_source(task_entity, queries, shadow) {
                return false;
            }
            let assigned_task = AssignedTask::Gather(crate::systems::soul_ai::task_execution::types::GatherData {
                target: task_entity,
                work_type,
                phase: GatherPhase::GoingToResource,
            });
            let reservation_ops = vec![
                ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
            ];
            shadow.apply_reserve_ops(&reservation_ops);
            queries.assignment_writer.write(TaskAssignmentRequest {
                familiar_entity: fam_entity,
                worker_entity,
                task_entity,
                work_type,
                task_pos,
                assigned_task,
                reservation_ops,
                already_commanded: uc_opt.is_some(),
            });
            return true;
        }
        WorkType::HaulToMixer => {
            // 固体原料（Sand/Rock）をミキサーへ運ぶ専用タスク
            let target_mixer = queries.target_mixers.get(task_entity).ok().map(|tm| tm.0);
            let item_info = queries.items.get(task_entity).ok().map(|(it, _)| it.0);

            let Some(mixer_entity) = target_mixer else {
                debug!("ASSIGN: HaulToMixer task {:?} has no TargetMixer", task_entity);
                return false;
            };

            let Some(item_type) = item_info else {
                debug!("ASSIGN: HaulToMixer item {:?} has no ResourceItem", task_entity);
                return false;
            };

            // ソース予約チェック
            let current_reserved = queries.resource_cache.get_source_reservation(task_entity)
                + shadow.source_reserved(task_entity);
            if current_reserved > 0 {
                debug!("ASSIGN: HaulToMixer item {:?} is already reserved", task_entity);
                return false;
            }

            // ミキサーが受け入れ可能かチェック
            let can_accept = if let Ok((_, storage, _)) = queries.mixers.get(mixer_entity) {
                let reserved = queries.resource_cache.get_mixer_destination_reservation(mixer_entity, item_type)
                    + shadow.mixer_reserved(mixer_entity, item_type);
                storage.can_accept(item_type, (1 + reserved) as u32)
            } else {
                false
            };

            if !can_accept {
                debug!("ASSIGN: Mixer {:?} cannot accept item {:?} (Full or Reserved)", mixer_entity, item_type);
                return false;
            }

            let assigned_task = AssignedTask::HaulToMixer(crate::systems::soul_ai::task_execution::types::HaulToMixerData {
                item: task_entity,
                mixer: mixer_entity,
                resource_type: item_type,
                phase: crate::systems::soul_ai::task_execution::types::HaulToMixerPhase::GoingToItem,
            });
            let reservation_ops = vec![
                ResourceReservationOp::ReserveMixerDestination {
                    target: mixer_entity,
                    resource_type: item_type,
                },
                ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
            ];
            shadow.apply_reserve_ops(&reservation_ops);
            queries.assignment_writer.write(TaskAssignmentRequest {
                familiar_entity: fam_entity,
                worker_entity,
                task_entity,
                work_type: WorkType::HaulToMixer,
                task_pos,
                assigned_task,
                reservation_ops,
                already_commanded: uc_opt.is_some(),
            });
            return true;
        }
        WorkType::Haul => {
            if let Ok(target_bp) = queries.target_blueprints.get(task_entity) {
                // ソース予約チェック
                let current_reserved = queries.resource_cache.get_source_reservation(task_entity)
                    + shadow.source_reserved(task_entity);
                if current_reserved > 0 {
                    debug!("ASSIGN: Item {:?} (for BP) is already reserved", task_entity);
                    return false;
                }

                let assigned_task = AssignedTask::HaulToBlueprint(crate::systems::soul_ai::task_execution::types::HaulToBlueprintData {
                    item: task_entity,
                    blueprint: target_bp.0,
                    phase: HaulToBpPhase::GoingToItem,
                });
                let reservation_ops = vec![
                    ResourceReservationOp::ReserveDestination { target: target_bp.0 },
                    ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
                ];
                shadow.apply_reserve_ops(&reservation_ops);
                queries.assignment_writer.write(TaskAssignmentRequest {
                    familiar_entity: fam_entity,
                    worker_entity,
                    task_entity,
                    work_type: WorkType::Haul,
                    task_pos,
                    assigned_task,
                    reservation_ops,
                    already_commanded: uc_opt.is_some(),
                });
                return true;
            }

            // ソース予約チェック (一般Item)
            let current_reserved = queries.resource_cache.get_source_reservation(task_entity)
                + shadow.source_reserved(task_entity);
            if current_reserved > 0 {
                debug!("ASSIGN: Item {:?} is already reserved", task_entity);
                return false;
            }

            let item_info = queries.items.get(task_entity).ok().map(|(it, _)| it.0);
            let item_belongs = queries.belongs.get(task_entity).ok();

            if item_info.is_none() {
                debug!("ASSIGN: Haul item {:?} has no ResourceItem", task_entity);
                return false;
            }
            let item_type = item_info.unwrap();


            let best_stockpile = queries.stockpiles
                .iter()
                .filter(|(s_entity, s_transform, stock, stored)| {
                    if let Some(area) = task_area_opt {
                        if !area.contains(s_transform.translation.truncate()) {
                            return false;
                        }
                    }

                    // 所有権チェック
                    let stock_belongs = queries.belongs.get(*s_entity).ok();
                    if item_belongs != stock_belongs {
                        // debug!("ASSIGN: Stockpile {:?} rejected due to ownership mismatch. Item: {:?}, Stock: {:?}", s_entity, item_belongs, stock_belongs);
                        return false;
                    }

                    // 専用ストレージ（所有権あり）かつバケツなら、型不一致でも許可する
                    // これにより、BucketEmpty専用になった置き場にBucketWaterを戻せる（逆も然り）
                    let is_dedicated = stock_belongs.is_some();
                    let is_bucket = matches!(item_type, ResourceType::BucketEmpty | ResourceType::BucketWater);
                    
                    let type_match = if is_dedicated && is_bucket {
                        true
                    } else {
                        stock.resource_type.is_none() || stock.resource_type == Some(item_type)
                    };

                    if !type_match {
                        // debug!("ASSIGN: Stockpile {:?} rejected due to type mismatch. StockType: {:?}, ItemType: {:?}", s_entity, stock.resource_type, item_type);
                    }

                    let current_count = stored.map(|s| s.len()).unwrap_or(0);
                    let reserved = queries.resource_cache.get_destination_reservation(*s_entity)
                        + shadow.destination_reserved(*s_entity);
                    let has_capacity = (current_count + reserved) < stock.capacity as usize;

                    type_match && has_capacity
                })
                .min_by(|(_, t1, _, _), (_, t2, _, _)| {
                    let d1 = t1.translation.truncate().distance_squared(task_pos);
                    let d2 = t2.translation.truncate().distance_squared(task_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, _, _, _)| e);

            if let Some(stock_entity) = best_stockpile {
                let assigned_task = AssignedTask::Haul(crate::systems::soul_ai::task_execution::types::HaulData {
                    item: task_entity,
                    stockpile: stock_entity,
                    phase: HaulPhase::GoingToItem,
                });
                let reservation_ops = vec![
                    ResourceReservationOp::ReserveDestination { target: stock_entity },
                    ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
                ];
                shadow.apply_reserve_ops(&reservation_ops);
                queries.assignment_writer.write(TaskAssignmentRequest {
                    familiar_entity: fam_entity,
                    worker_entity,
                    task_entity,
                    work_type: WorkType::Haul,
                    task_pos,
                    assigned_task,
                    reservation_ops,
                    already_commanded: uc_opt.is_some(),
                });
                return true;
            }
            debug!("ASSIGN: No suitable stockpile found for item {:?} (type: {:?})", task_entity, item_type);
            return false;
        }
        WorkType::Build => {
            if let Ok((_, bp, _)) = queries.blueprints.get(task_entity) {
                if !bp.materials_complete() {
                    debug!("ASSIGN: Build target {:?} materials not complete", task_entity);
                    return false;
                }
            }

            // 建築タスクもソース予約として管理（TaskSlotsで人数制限）
            if !can_reserve_source(task_entity, queries, shadow) {
                return false;
            }
            let assigned_task = AssignedTask::Build(crate::systems::soul_ai::task_execution::types::BuildData {
                blueprint: task_entity,
                phase: BuildPhase::GoingToBlueprint,
            });
            let reservation_ops = vec![
                ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
            ];
            shadow.apply_reserve_ops(&reservation_ops);
            queries.assignment_writer.write(TaskAssignmentRequest {
                familiar_entity: fam_entity,
                worker_entity,
                task_entity,
                work_type: WorkType::Build,
                task_pos,
                assigned_task,
                reservation_ops,
                already_commanded: uc_opt.is_some(),
            });
            return true;
        }
        WorkType::GatherWater => {
            // バケツ予約チェック
            let current_reserved = queries.resource_cache.get_source_reservation(task_entity)
                + shadow.source_reserved(task_entity);
            if current_reserved > 0 {
                return false;
            }

            let best_tank = queries.stockpiles
                .iter()
                .filter(|(s_entity, s_transform, stock, stored)| {
                    if let Some(area) = task_area_opt {
                        if !area.contains(s_transform.translation.truncate()) {
                            return false;
                        }
                    }
                    let is_tank = stock.resource_type == Some(ResourceType::Water);
                    let current_water = stored.map(|s| s.len()).unwrap_or(0);
                    let reserved_tank = queries.resource_cache.get_destination_reservation(*s_entity)
                        + shadow.destination_reserved(*s_entity);
                     // タンクに関しては、何人（何個）が集まっているか、という予約になっている
                     // バケツ1つあたり5の水が入るので、本当は容量チェックをシビアにするべきだが
                     // ここでは destination_reservation (人数) でチェックする
                    let has_capacity = (current_water + reserved_tank) < stock.capacity;

                    // 所有権チェック（バケツとタンク）
                    let bucket_belongs = queries.belongs.get(task_entity).ok();
                    let _tank_belongs = Some(&crate::systems::logistics::BelongsTo(*s_entity)); // タンク自身への帰属
                    
                    let is_my_tank = bucket_belongs.map(|b| b.0) == Some(*s_entity);

                    is_tank && has_capacity && is_my_tank
                })
                .min_by(|(_, t1, _, _), (_, t2, _, _)| {
                    let d1 = t1.translation.truncate().distance_squared(task_pos);
                    let d2 = t2.translation.truncate().distance_squared(task_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, _, _, _)| e);

            if let Some(tank_entity) = best_tank {
                let assigned_task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                    bucket: task_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::GoingToBucket,
                });
                let reservation_ops = vec![
                    ResourceReservationOp::ReserveDestination { target: tank_entity },
                    ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
                ];
            shadow.apply_reserve_ops(&reservation_ops);
                queries.assignment_writer.write(TaskAssignmentRequest {
                    familiar_entity: fam_entity,
                    worker_entity,
                    task_entity,
                    work_type: WorkType::GatherWater,
                    task_pos,
                    assigned_task,
                    reservation_ops,
                    already_commanded: uc_opt.is_some(),
                });
                return true;
            }
            debug!("ASSIGN: No suitable tank/mixer found for bucket {:?}", task_entity);
            return false;
        }
        WorkType::CollectSand => {
            if !can_reserve_source(task_entity, queries, shadow) {
                return false;
            }
            let assigned_task = AssignedTask::CollectSand(crate::systems::soul_ai::task_execution::types::CollectSandData {
                target: task_entity,
                phase: crate::systems::soul_ai::task_execution::types::CollectSandPhase::GoingToSand,
            });
            let reservation_ops = vec![
                ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
            ];
            shadow.apply_reserve_ops(&reservation_ops);
            queries.assignment_writer.write(TaskAssignmentRequest {
                familiar_entity: fam_entity,
                worker_entity,
                task_entity,
                work_type,
                task_pos,
                assigned_task,
                reservation_ops,
                already_commanded: uc_opt.is_some(),
            });
            return true;
        }
        WorkType::Refine => {
            if !can_reserve_source(task_entity, queries, shadow) {
                return false;
            }
            let assigned_task = AssignedTask::Refine(crate::systems::soul_ai::task_execution::types::RefineData {
                mixer: task_entity,
                phase: crate::systems::soul_ai::task_execution::types::RefinePhase::GoingToMixer,
            });
            let reservation_ops = vec![
                ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
            ];
            shadow.apply_reserve_ops(&reservation_ops);
            queries.assignment_writer.write(TaskAssignmentRequest {
                familiar_entity: fam_entity,
                worker_entity,
                task_entity,
                work_type,
                task_pos,
                assigned_task,
                reservation_ops,
                already_commanded: uc_opt.is_some(),
            });
            return true;
        }
        WorkType::HaulWaterToMixer => {
            // TargetMixer があるか確認
            let target_mixer = queries.target_mixers.get(task_entity).ok().map(|tm| tm.0);
            let mixer_entity = if let Some(m) = target_mixer { m } else {
                debug!("ASSIGN: HaulWaterToMixer task {:?} has no TargetMixer", task_entity);
                return false;
            };

            // バケツの BelongsTo から Tank を取得
            let tank_entity = if let Ok(belongs) = queries.belongs.get(task_entity) {
                belongs.0
            } else {
                debug!("ASSIGN: HaulWaterToMixer bucket {:?} has no BelongsTo (Tank)", task_entity);
                return false;
            };

            // ソース（バケツ）予約チェック
            let current_reserved = queries.resource_cache.get_source_reservation(task_entity)
                + shadow.source_reserved(task_entity);
            if current_reserved > 0 {
                return false;
            }
            
            let assigned_task = AssignedTask::HaulWaterToMixer(crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
                bucket: task_entity,
                tank: tank_entity,
                mixer: mixer_entity,
                amount: 0,
                phase: crate::systems::soul_ai::task_execution::types::HaulWaterToMixerPhase::GoingToBucket,
            });
            let reservation_ops = vec![
                ResourceReservationOp::ReserveMixerDestination {
                    target: mixer_entity,
                    resource_type: ResourceType::Water,
                },
                ResourceReservationOp::ReserveSource { source: task_entity, amount: 1 },
            ];
            shadow.apply_reserve_ops(&reservation_ops);
            queries.assignment_writer.write(TaskAssignmentRequest {
                familiar_entity: fam_entity,
                worker_entity,
                task_entity,
                work_type: WorkType::HaulWaterToMixer,
                task_pos,
                assigned_task,
                reservation_ops,
                already_commanded: uc_opt.is_some(),
            });
            return true;
        }
    }
}
