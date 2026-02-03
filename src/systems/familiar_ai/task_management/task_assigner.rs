//! タスク割り当てモジュール
//!
//! ワーカーへのタスク割り当てロジックを提供します。

use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::UnderCommand;
use crate::events::{OnSoulRecruited, OnTaskAssigned};
use crate::systems::command::TaskArea;
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::types::{
    AssignedTask, BuildPhase, GatherPhase, GatherWaterPhase, HaulPhase, HaulToBpPhase,
};

use bevy::prelude::*;

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
        UnderCommand(fam_entity),
        crate::relationships::WorkingOn(task_entity),
    ));
    commands
        .entity(task_entity)
        .insert(crate::systems::jobs::IssuedBy(fam_entity));
}

/// ワーカーにタスクを割り当てる
#[allow(clippy::too_many_arguments)]
pub fn assign_task_to_worker(
    commands: &mut Commands,
    fam_entity: Entity,
    task_entity: Entity,
    worker_entity: Entity,
    fatigue_threshold: f32,
    queries: &crate::systems::soul_ai::task_execution::context::TaskQueries,
    q_souls: &mut Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,

            Option<&mut crate::systems::logistics::Inventory>,
            Option<&UnderCommand>,
            Option<&ParticipatingIn>,
        ),
        Without<crate::entities::familiar::Familiar>,
    >,
    task_area_opt: Option<&TaskArea>,
    haul_cache: &mut crate::systems::familiar_ai::resource_cache::SharedResourceCache,
) -> bool {
    let Ok((_, _, soul, mut assigned_task, mut dest, mut path, idle, _, uc_opt, participating_opt)) =
        q_souls.get_mut(worker_entity)
    else {
        warn!("ASSIGN: Worker {:?} not found in query", worker_entity);
        return false;
    };

    // もし集会に参加中なら抜ける
    if let Some(p) = participating_opt {
        commands.entity(worker_entity).remove::<ParticipatingIn>();
        commands.trigger(crate::events::OnGatheringLeft {
            entity: worker_entity,
            spot_entity: p.0,
        });
    }

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
            prepare_worker_for_task(
                commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
            );

            *assigned_task = AssignedTask::Gather(crate::systems::soul_ai::task_execution::types::GatherData {
                target: task_entity,
                work_type,
                phase: GatherPhase::GoingToResource,
            });
            dest.0 = task_pos;
            path.waypoints.clear();
            path.current_index = 0;
            commands.trigger(OnTaskAssigned {
                entity: worker_entity,
                task_entity,
                work_type,
            });
            return true;
        }
        WorkType::Haul => {
            if let Ok(target_bp) = queries.target_blueprints.get(task_entity) {
                // ソース予約チェック
                if haul_cache.get_source_reservation(task_entity) > 0 {
                    debug!("ASSIGN: Item {:?} (for BP) is already reserved", task_entity);
                    return false;
                }

                haul_cache.reserve_destination(target_bp.0);
                haul_cache.reserve_source(task_entity, 1);

                prepare_worker_for_task(
                    commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
                );

                *assigned_task = AssignedTask::HaulToBlueprint(crate::systems::soul_ai::task_execution::types::HaulToBlueprintData {
                    item: task_entity,
                    blueprint: target_bp.0,
                    phase: HaulToBpPhase::GoingToItem,
                });
                dest.0 = task_pos;
                path.waypoints.clear();
                path.current_index = 0;
                commands.trigger(OnTaskAssigned {
                    entity: worker_entity,
                    task_entity,
                    work_type: WorkType::Haul,
                });
                return true;
            }

            // ソース予約チェック (一般Item)
            if haul_cache.get_source_reservation(task_entity) > 0 {
                debug!("ASSIGN: Item {:?} is already reserved", task_entity);
                return false;
            }

            let item_info = queries.items.get(task_entity).ok().map(|(it, _)| it.0);
            let item_belongs = queries.belongs.get(task_entity).ok();
            let target_mixer = queries.target_mixers.get(task_entity).ok().map(|tm| tm.0);

            if item_info.is_none() {
                debug!("ASSIGN: Haul item {:?} has no ResourceItem", task_entity);
                return false;
            }
            let item_type = item_info.unwrap();

            // ミキサーが指定されている場合
            if let Some(mixer_entity) = target_mixer {
                // ミキサーが受け入れ可能なリソース（砂または岩）であるか確認
                if matches!(item_type, ResourceType::Sand | ResourceType::Rock) {
                    // ミキサーのキャパシティチェック
                    let can_accept = if let Ok((_, storage, _)) = queries.mixers.get(mixer_entity) {
                        let reserved = haul_cache.get_mixer_destination_reservation(mixer_entity, item_type);
                        storage.can_accept(item_type, (1 + reserved) as u32)
                    } else {
                        false
                    };

                    if can_accept {
                        haul_cache.reserve_mixer_destination(mixer_entity, item_type);
                        haul_cache.reserve_source(task_entity, 1);

                        prepare_worker_for_task(
                            commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
                        );
                        *assigned_task = AssignedTask::HaulToMixer(crate::systems::soul_ai::task_execution::types::HaulToMixerData {
                            item: task_entity,
                            mixer: mixer_entity,
                            resource_type: item_type,
                            phase: crate::systems::soul_ai::task_execution::types::HaulToMixerPhase::GoingToItem,
                        });
                        dest.0 = task_pos;
                        path.waypoints.clear();
                        path.current_index = 0;
                        commands.trigger(OnTaskAssigned {
                            entity: worker_entity,
                            task_entity,
                            work_type: WorkType::Haul,
                        });
                        return true;
                    } else {
                        debug!("ASSIGN: Mixer {:?} cannot accept item {:?} (Full or Reserved)", mixer_entity, item_type);
                    }
                } else {
                    debug!("ASSIGN: Haul item {:?} has TargetMixer but type {:?} is not accepted as solid material", task_entity, item_type);
                    // ここで TargetMixer を削除してしまうか、あるいはミキサー用タスクではないとして通常の運搬に回す
                    // 今回は通常の運搬（ストックパイル行き）へのフォールバックを許容するため、あえて TargetMixer は消さず、
                    // 下の通常の Haul ロジックに進ませる
                }
            }



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
                    let reserved = haul_cache.get_destination_reservation(*s_entity);
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
                haul_cache.reserve_destination(stock_entity);
                haul_cache.reserve_source(task_entity, 1);

                prepare_worker_for_task(
                    commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
                );

                *assigned_task = AssignedTask::Haul(crate::systems::soul_ai::task_execution::types::HaulData {
                    item: task_entity,
                    stockpile: stock_entity,
                    phase: HaulPhase::GoingToItem,
                });
                
                dest.0 = task_pos;
                path.waypoints.clear();
                path.current_index = 0;
                commands.trigger(OnTaskAssigned {
                    entity: worker_entity,
                    task_entity,
                    work_type: WorkType::Haul,
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

            prepare_worker_for_task(
                commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
            );

            *assigned_task = AssignedTask::Build(crate::systems::soul_ai::task_execution::types::BuildData {
                blueprint: task_entity,
                phase: BuildPhase::GoingToBlueprint,
            });
            dest.0 = task_pos;
            path.waypoints.clear();
            path.current_index = 0;
            commands.trigger(OnTaskAssigned {
                entity: worker_entity,
                task_entity,
                work_type: WorkType::Build,
            });
            return true;
        }
        WorkType::GatherWater => {
            // バケツ予約チェック
            if haul_cache.get_source_reservation(task_entity) > 0 {
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
                    let reserved_tank = haul_cache.get_destination_reservation(*s_entity);
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
                haul_cache.reserve_destination(tank_entity);
                haul_cache.reserve_source(task_entity, 1);

                prepare_worker_for_task(
                    commands,
                    worker_entity,
                    fam_entity,
                    task_entity,
                    uc_opt.is_some(),
                );

                *assigned_task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                    bucket: task_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::GoingToBucket,
                });

                dest.0 = task_pos;
                path.waypoints.clear();
                path.current_index = 0;
                commands.trigger(OnTaskAssigned {
                    entity: worker_entity,
                    task_entity,
                    work_type: WorkType::GatherWater,
                });
                return true;
            }
            debug!("ASSIGN: No suitable tank/mixer found for bucket {:?}", task_entity);
            return false;
        }
        WorkType::CollectSand => {
            prepare_worker_for_task(
                commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
            );

            *assigned_task = AssignedTask::CollectSand(crate::systems::soul_ai::task_execution::types::CollectSandData {
                target: task_entity,
                phase: crate::systems::soul_ai::task_execution::types::CollectSandPhase::GoingToSand,
            });
            dest.0 = task_pos;
            path.waypoints.clear();
            path.current_index = 0;
            commands.trigger(OnTaskAssigned {
                entity: worker_entity,
                task_entity,
                work_type,
            });
            return true;
        }
        WorkType::Refine => {
            prepare_worker_for_task(
                commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
            );

            *assigned_task = AssignedTask::Refine(crate::systems::soul_ai::task_execution::types::RefineData {
                mixer: task_entity,
                phase: crate::systems::soul_ai::task_execution::types::RefinePhase::GoingToMixer,
            });
            dest.0 = task_pos;
            path.waypoints.clear();
            path.current_index = 0;
            commands.trigger(OnTaskAssigned {
                entity: worker_entity,
                task_entity,
                work_type,
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
            if haul_cache.get_source_reservation(task_entity) > 0 {
                return false;
            }
            
            haul_cache.reserve_mixer_destination(mixer_entity, ResourceType::Water);
            haul_cache.reserve_source(task_entity, 1);

            prepare_worker_for_task(
                commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
            );

            *assigned_task = AssignedTask::HaulWaterToMixer(crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
                bucket: task_entity,
                tank: tank_entity,
                mixer: mixer_entity,
                amount: 0,
                phase: crate::systems::soul_ai::task_execution::types::HaulWaterToMixerPhase::GoingToBucket,
            });
            
            dest.0 = task_pos;
            path.waypoints.clear();
            path.current_index = 0;
            commands.trigger(OnTaskAssigned {
                entity: worker_entity,
                task_entity,
                work_type: WorkType::HaulWaterToMixer,
            });
            return true;
        }
    }
}
