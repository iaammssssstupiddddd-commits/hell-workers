//! MudMixer auto-haul system
//!
//! Automatically creates haul tasks for materials needed by MudMixer.

use bevy::prelude::*;

use crate::constants::{MUD_MIXER_CAPACITY, BUCKET_CAPACITY, TILE_SIZE};
use crate::entities::familiar::ActiveCommand;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, MudMixerStorage, TargetMixer, TaskSlots, WorkType};
use crate::systems::logistics::{ResourceItem, ResourceType, Stockpile};
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps};
use crate::relationships::TaskWorkers;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;


/// MudMixer への自動資材運搬タスク生成システム
pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
    mut haul_cache: ResMut<HaulReservationCache>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_mixers: Query<(Entity, &Transform, &MudMixerStorage, Option<&TaskWorkers>)>,
    q_resources_with_belongs: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            Option<&crate::systems::logistics::BelongsTo>,
            Option<&crate::relationships::StoredIn>,
        ),
        (Without<Designation>, Without<TaskWorkers>),
    >,
    q_stockpiles_detailed: Query<(Entity, &Transform, &Stockpile, Option<&crate::relationships::StoredItems>)>,
    q_sandpiles: Query<(Entity, &Transform, &crate::systems::logistics::BelongsTo), (With<crate::systems::jobs::SandPile>, Without<Designation>)>,
) {
    let mut already_assigned_this_frame = std::collections::HashSet::new();


    for (_fam_entity, _active_command, task_area) in q_familiars.iter() {
        for (mixer_entity, mixer_transform, storage, _workers_opt) in q_mixers.iter() {
            let mixer_pos = mixer_transform.translation.truncate();
            if !task_area.contains(mixer_pos) {
                continue;
            }

            // 各リソースについてチェック
            let resources_to_check = [ResourceType::Sand, ResourceType::Rock];
            for resource_type in resources_to_check {
                let current = match resource_type {
                    ResourceType::Sand => storage.sand,
                    ResourceType::Rock => storage.rock,
                    _ => 0,
                };
                let inflight_count = haul_cache.get_mixer(mixer_entity, resource_type);

                // 満杯ならスキップ
                if current >= MUD_MIXER_CAPACITY {
                    debug!("AUTO_HAUL_MIXER: Mixer {:?} is full for {:?} (current={}), skipping", mixer_entity, resource_type, current);
                    continue;
                }

                if current + (inflight_count as u32) >= MUD_MIXER_CAPACITY {
                    debug!("AUTO_HAUL_MIXER: Mixer {:?} has enough {:?} in-flight (current={}, inflight={}), skipping", mixer_entity, resource_type, current, inflight_count);
                    continue;
                }


                // 近場のアイテムを探す
                let search_radius = TILE_SIZE * 30.0;
                let nearby = resource_grid.get_nearby_in_radius(mixer_pos, search_radius);

                debug!("AUTO_HAUL_MIXER: Searching {:?} for Mixer {:?}, current={}, inflight={}, nearby_count={}",
                      resource_type, mixer_entity, current, inflight_count, nearby.len());


                let matching = nearby.into_iter()
                    .filter(|&e| !already_assigned_this_frame.contains(&e))
                    .filter_map(|e| {
                        let query_result = q_resources_with_belongs.get(e);
                        if query_result.is_err() {
                            return None;
                        }
                        let (_, transform, vis, res_item, _belongs, stored_in_opt) = query_result.unwrap();
                        if *vis == Visibility::Hidden {
                            return None;
                        }
                        if res_item.0 != resource_type {
                            return None;
                        }

                        // 既に Reserved されているものはクエリの Without<Designation> で除外済み

                        if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                            if let Ok((_, stock_transform, _, _)) = q_stockpiles_detailed.get(*stock_entity) {
                                if !task_area.contains(stock_transform.translation.truncate()) { return None; }
                            }
                        }

                        Some((e, transform.translation.truncate().distance_squared(mixer_pos)))
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .map(|a| a.0);

                if let Some(item_entity) = matching {
                    already_assigned_this_frame.insert(item_entity);
                    haul_cache.reserve_mixer(mixer_entity, resource_type);

                    debug!("AUTO_HAUL_MIXER: Issuing HaulToMixer for {:?} ({:?}) to Mixer {:?}",
                          item_entity, resource_type, mixer_entity);

                    commands.entity(item_entity).insert((
                        Designation { work_type: WorkType::Haul },
                        TargetMixer(mixer_entity),
                        TaskSlots::new(1),
                        IssuedBy(_fam_entity),
                    ));
                } else if resource_type == ResourceType::Sand {
                    // 砂アイテムが見つからない場合、SandPile への CollectSand タスクを発行
                    let sandpile = q_sandpiles.iter()
                        .find(|(_, _, belongs)| belongs.0 == mixer_entity);

                    if let Some((sandpile_entity, _, _)) = sandpile {
                        if !already_assigned_this_frame.contains(&sandpile_entity) {
                            already_assigned_this_frame.insert(sandpile_entity);

                            debug!("AUTO_HAUL_MIXER: Issuing CollectSand for SandPile {:?} to Mixer {:?}",
                                  sandpile_entity, mixer_entity);

                            commands.entity(sandpile_entity).insert((
                                Designation { work_type: WorkType::CollectSand },
                                TaskSlots::new(1),
                                IssuedBy(_fam_entity),
                            ));
                        }
                    } else {
                        debug!("AUTO_HAUL_MIXER: No SandPile found for Mixer {:?}", mixer_entity);
                    }
                } else {
                    debug!("AUTO_HAUL_MIXER: No matching {:?} item found for Mixer {:?}", resource_type, mixer_entity);
                }

            }

            // Water の自動リクエスト

            let water_current = storage.water;
            let water_inflight = haul_cache.get_mixer(mixer_entity, ResourceType::Water) as u32;
            
            if water_current + (water_inflight * BUCKET_CAPACITY) < MUD_MIXER_CAPACITY {
                // TaskArea内のTankを探す
                let mut tank_with_water = None;
                for (stock_entity, stock_transform, stock, stored_opt) in q_stockpiles_detailed.iter() {
                    if stock.resource_type == Some(ResourceType::Water) {
                        if task_area.contains(stock_transform.translation.truncate()) {
                            let water_count = stored_opt.map(|s| s.len()).unwrap_or(0);
                            if water_count > 0 {
                                tank_with_water = Some(stock_entity);
                                break;
                            }
                        }
                    }
                }

                if let Some(tank_entity) = tank_with_water {
                    // そのTank専用の空バケツを探す（tank_water_requestと同様のロジック）
                    let mut found_bucket = None;
                    for (e, transform, vis, res_item, belongs_opt, stored_in_opt) in q_resources_with_belongs.iter() {
                        // 非表示はスキップ
                        if *vis == Visibility::Hidden { continue; }
                        // 空バケツのみ対象
                        if res_item.0 != ResourceType::BucketEmpty { continue; }
                        // 既に割り当て済みはスキップ
                        if already_assigned_this_frame.contains(&e) { continue; }
                        // StoredInがない（持ち運び中など）はスキップ
                        if stored_in_opt.is_none() { continue; }

                        // BelongsToでこのタンクに紐付いたバケツのみ
                        if let Some(belongs) = belongs_opt {
                            if belongs.0 == tank_entity {
                                found_bucket = Some((e, transform.translation.truncate().distance_squared(mixer_pos)));
                                break; // 専用バケツ優先
                            }
                        }
                    }

                    if let Some(bucket_entity) = found_bucket.map(|(e, _)| e) {
                        already_assigned_this_frame.insert(bucket_entity);
                        haul_cache.reserve_mixer(mixer_entity, ResourceType::Water);

                        commands.entity(bucket_entity).insert((
                            Designation { work_type: WorkType::HaulWaterToMixer },
                            TargetMixer(mixer_entity),
                            TaskSlots::new(1),
                            IssuedBy(_fam_entity),
                            crate::systems::jobs::Priority(6), // 通常の運搬より優先
                        ));
                        debug!("AUTO_HAUL_MIXER: Issued HaulWaterToMixer for bucket {:?} from Tank {:?} to Mixer {:?}", bucket_entity, tank_entity, mixer_entity);
                    }
                }
            }

        }
    }
}
