//! MudMixer auto-haul system
//!
//! Automatically creates haul tasks for materials needed by MudMixer.

use bevy::prelude::*;

use crate::constants::{MUD_MIXER_CAPACITY, BUCKET_CAPACITY, TILE_SIZE};
use crate::entities::familiar::ActiveCommand;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, MudMixerStorage, TargetMixer, TaskSlots, WorkType, Priority};
use crate::systems::logistics::{ResourceItem, ResourceType, ReservedForTask, Stockpile};
use crate::relationships::TaskWorkers;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::soul_ai::work::auto_haul::ItemReservations;
use crate::systems::soul_ai::task_execution::AssignedTask;

/// MudMixer への自動資材運搬タスク生成システム
pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    mut haul_cache: ResMut<HaulReservationCache>,
    mut item_reservations: ResMut<ItemReservations>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_mixers: Query<(Entity, &Transform, &MudMixerStorage, Option<&TaskWorkers>)>,
    q_stockpiles_detailed: Query<(Entity, &Transform, &Stockpile, Option<&crate::relationships::StoredItems>)>,
    q_souls: Query<&AssignedTask>,
    q_resources_with_belongs: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            Option<&crate::systems::logistics::BelongsTo>,
            Option<&crate::relationships::StoredIn>,
            Option<&ReservedForTask>,
            Option<&Designation>,
            Option<&TaskWorkers>,
        ),
    >,
    q_sand_piles: Query<(Entity, &Transform, Option<&Designation>, Option<&TaskWorkers>), With<crate::systems::jobs::SandPile>>,
) {
    let mut already_assigned_this_frame = std::collections::HashSet::new();
    let mut water_inflight_by_mixer = std::collections::HashMap::<Entity, usize>::new();

    for task in q_souls.iter() {
        if let AssignedTask::HaulWaterToMixer(data) = task {
            *water_inflight_by_mixer.entry(data.mixer).or_insert(0) += 1;
        }
    }

    for (_fam_entity, _active_command, task_area) in q_familiars.iter() {
        for (mixer_entity, mixer_transform, storage, _workers_opt) in q_mixers.iter() {
            // TaskArea外のミキサーはスキップ
            if !task_area.contains(mixer_transform.translation.truncate()) {
                continue;
            }

            // 他の使い魔の領域リストを取得
            let other_areas: Vec<&TaskArea> = q_familiars
                .iter()
                .filter(|(e, _, _)| *e != _fam_entity)
                .map(|(_, _, area)| area)
                .collect();

            // --- 固体原料の自動運搬 (Sand, Rock) ---
            let resources_to_check = [ResourceType::Sand, ResourceType::Rock];
            for resource_type in resources_to_check {
                let current = match resource_type {
                    ResourceType::Sand => storage.sand,
                    ResourceType::Rock => storage.rock,
                    _ => 0,
                };
                let inflight_count = haul_cache.get_mixer(mixer_entity, resource_type);

                // 満杯ならスキップ
                if storage.is_full(resource_type) {
                    continue;
                }

                if current + (inflight_count as u32) >= MUD_MIXER_CAPACITY {
                    continue;
                }

                // 運搬可能なアイテムを探す（全域検索・距離ソート・他領域Stockpile/Sandpile除外）
                let mixer_pos = mixer_transform.translation.truncate();
                
                let mut candidates = Vec::new();
                for (e, transform, vis, res_item, _belongs, stored_in_opt, reserved_opt, designation, workers) in q_resources_with_belongs.iter() {
                    if *vis == Visibility::Hidden { continue; }
                    if res_item.0 != resource_type { continue; }
                    
                    // すでに仕事（DesignationやWorkers）があるものはスキップ
                    if designation.is_some() || workers.is_some() { continue; }
                    if reserved_opt.is_some() { continue; }
                    if already_assigned_this_frame.contains(&e) { continue; }
                    if item_reservations.0.contains(&e) { continue; }

                    // 倉庫に入っている場合、その倉庫が他者のエリア内ならスキップ
                    if let Some(stored_in) = stored_in_opt {
                        if let Ok((_, stock_transform, _, _)) = q_stockpiles_detailed.get(stored_in.0) {
                            let stock_pos = stock_transform.translation.truncate();
                            if other_areas.iter().any(|area| area.contains(stock_pos)) {
                                continue;
                            }
                        }
                        // SandPileなどの特殊倉庫も考慮（q_stockpiles_detailedに含まれていない場合）
                        // 現状 SandPile は ResourceItem 化されていないはずだが、念のため
                        if let Ok((_, stock_transform, _, _)) = q_sand_piles.get(stored_in.0) {
                            let stock_pos = stock_transform.translation.truncate();
                            if other_areas.iter().any(|area| area.contains(stock_pos)) {
                                continue;
                            }
                        }
                    }

                    let item_pos = transform.translation.truncate();
                    let dist_sq = item_pos.distance_squared(mixer_pos);
                    candidates.push((e, dist_sq));
                }

                // 近い順にソート
                candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                if let Some((item_entity, _)) = candidates.first() {
                    let item_entity = *item_entity;
                    commands.entity(item_entity).insert((
                        Designation { work_type: WorkType::Haul },
                        TargetMixer(mixer_entity),
                        TaskSlots::new(1),
                        IssuedBy(_fam_entity),
                        Priority(5),
                        ReservedForTask,
                    ));
                    haul_cache.reserve_mixer(mixer_entity, resource_type);
                    already_assigned_this_frame.insert(item_entity);
                    item_reservations.0.insert(item_entity);
                    info!("AUTO_HAUL_MIXER: Assigned {:?} haul to Mixer {:?}", resource_type, mixer_entity);
                }
            }
        

            // --- 砂採取タスクの自動発行 ---
            // 砂アイテムが足りず、かつミキサー近辺にSandPileがある場合
            if storage.sand + (haul_cache.get_mixer(mixer_entity, ResourceType::Sand) as u32) < 2 {
                for (sp_entity, sp_transform, sp_designation, sp_workers) in q_sand_piles.iter() {
                    // ミキサーの近く（3タイル以内）にあるSandPileを対象にする
                    let dist = sp_transform.translation.truncate().distance(mixer_transform.translation.truncate());
                    if dist < TILE_SIZE * 3.0 && task_area.contains(sp_transform.translation.truncate()) {
                        // 既にこのSandPileに仕事があるか確認
                        let has_designation = sp_designation.is_some() || sp_workers.is_some();
                        if !has_designation {
                            commands.entity(sp_entity).insert((
                                Designation { work_type: WorkType::CollectSand },
                                IssuedBy(_fam_entity),
                                TaskSlots::new(1),
                                Priority(4),
                            ));
                            info!("AUTO_HAUL_MIXER: Issued CollectSand for Mixer {:?}", mixer_entity);
                            break;
                        }
                    }
                }
            }

            // --- 水の自動リクエスト ---
            let water_inflight = *water_inflight_by_mixer.get(&mixer_entity).unwrap_or(&0) as u32;
            let (water_current, water_capacity) = if let Ok((_, _, stock, stored_opt)) =
                q_stockpiles_detailed.get(mixer_entity)
            {
                if stock.resource_type == Some(ResourceType::Water) {
                    (stored_opt.map(|s| s.len()).unwrap_or(0) as u32, stock.capacity as u32)
                } else {
                    (0, MUD_MIXER_CAPACITY)
                }
            } else {
                (0, MUD_MIXER_CAPACITY)
            };
            let issue_threshold = water_capacity.saturating_sub(BUCKET_CAPACITY);
            
            if water_current < water_capacity
                && water_current + (water_inflight * BUCKET_CAPACITY) <= issue_threshold
            {
                // 他の使い魔の領域リストを取得（上のループで定義されているが、スコープが違う可能性があるため再利用または再定義）
                // ※ ここでは同じ関数内なので other_areas は有効だが、念のため再定義せず利用する。
                // ただし、ブロックが切れている可能性があるため、安全策として再取得コードを含めるか、
                // あるいは既にスコープ内にあるならそのまま使う。
                // 今の構造だと for resources_to_check ループの外に other_areas があるのでアクセス可能。

                // 全域から最適なタンクを探す（バケツ1杯分以上の水があるタンクのみ）
                let mixer_pos = mixer_transform.translation.truncate();
                let mut tank_candidates = Vec::new();

                for (stock_entity, stock_transform, stock, stored_opt) in q_stockpiles_detailed.iter() {
                    if stock.resource_type == Some(ResourceType::Water) {
                        let tank_pos = stock_transform.translation.truncate();

                        // タンクが他者のエリア内にあるならスキップ
                        if other_areas.iter().any(|area| area.contains(tank_pos)) {
                            continue;
                        }

                        let water_count = stored_opt.map(|s| s.len()).unwrap_or(0);
                        // バケツ1杯分以上の水がないとタスクを発行しない
                        if water_count >= BUCKET_CAPACITY as usize {
                            let dist_sq = tank_pos.distance_squared(mixer_pos);
                            tank_candidates.push((stock_entity, dist_sq));
                        }
                    }
                }

                // 一番近いタンクを選択
                tank_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                let tank_with_water = tank_candidates.first().map(|(e, _)| *e);

                if let Some(tank_entity) = tank_with_water {
                    // そのTank専用のバケツを探す（空バケツ優先、水入りバケツも対象）
                    // ここもTaskArea制限を外し、距離で選ぶ。ただし他者エリアの倉庫（バケツ置き場）は除外。
                    let mut bucket_candidates = Vec::new();

                for (e, transform, vis, res_item, belongs_opt, stored_in_opt, reserved_opt, designation, workers) in q_resources_with_belongs.iter() {
                        // 非表示や作業中のものはスキップ
                        if *vis == Visibility::Hidden || workers.is_some() { continue; }
                        
                        // バケツ以外はスキップ
                        if !matches!(res_item.0, ResourceType::BucketEmpty | ResourceType::BucketWater) { continue; }

                    if reserved_opt.is_some() { continue; }
                        
                        // すでにこのフレームでアサイン済みならスキップ
                    if already_assigned_this_frame.contains(&e) { continue; }
                    if item_reservations.0.contains(&e) { continue; }

                        // Designationがあるものはスキップ（上書きしない）
                        if designation.is_some() {
                            continue;
                        }

                        // 他者のエリアにある倉庫（バケツ置き場）に入っているならスキップ
                        if let Some(stored_in) = stored_in_opt {
                            if let Ok((_, stock_transform, _, _)) = q_stockpiles_detailed.get(stored_in.0) {
                                let stock_pos = stock_transform.translation.truncate();
                                if other_areas.iter().any(|area| area.contains(stock_pos)) {
                                    continue;
                                }
                            }
                        }

                        // BelongsToでこのタンクに紐付いたバケツのみ
                        if let Some(belongs) = belongs_opt {
                            if belongs.0 == tank_entity {
                                let item_pos = transform.translation.truncate();
                                let dist_sq = item_pos.distance_squared(mixer_pos);
                                bucket_candidates.push((e, dist_sq, res_item.0));
                            }
                        }
                    }

                    // 空バケツを優先しつつ、距離が近いものを選ぶ
                    // ソート順: Empty < Water, 近い < 遠い
                    bucket_candidates.sort_by(|a, b| {
                        let type_order_a = if a.2 == ResourceType::BucketEmpty { 0 } else { 1 };
                        let type_order_b = if b.2 == ResourceType::BucketEmpty { 0 } else { 1 };
                        match type_order_a.cmp(&type_order_b) {
                            std::cmp::Ordering::Equal => a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal),
                            other => other,
                        }
                    });

                    if let Some((bucket_entity, _, _)) = bucket_candidates.first() {
                        commands.entity(*bucket_entity).insert((
                            Designation {
                                work_type: WorkType::HaulWaterToMixer,
                            },
                            TargetMixer(mixer_entity),
                            TaskSlots::new(1),
                            IssuedBy(_fam_entity),
                            Priority(6), // 通常の運搬より優先
                            ReservedForTask,
                        ));
                        item_reservations.0.insert(*bucket_entity);
                        haul_cache.reserve_mixer(mixer_entity, ResourceType::Water);
                        already_assigned_this_frame.insert(*bucket_entity);
                        info!("AUTO_HAUL_MIXER: Issued HaulWaterToMixer for bucket {:?} (Mixer {:?})", bucket_entity, mixer_entity);
                    }
                }
            }
        }
    }
}
