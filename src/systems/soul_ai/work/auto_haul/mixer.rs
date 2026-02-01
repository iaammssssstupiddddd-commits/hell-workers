//! MudMixer auto-haul system
//!
//! Automatically creates haul tasks for materials needed by MudMixer.

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::familiar::ActiveCommand;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, MudMixerStorage, TargetMixer, TaskSlots, WorkType};
use crate::systems::logistics::{ResourceItem, ResourceType, Stockpile};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps, SpatialGrid};
use crate::relationships::TaskWorkers;

/// MudMixer への自動資材運搬タスク生成システム
pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
    _tank_grid: Res<SpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_mixers: Query<(Entity, &Transform, &MudMixerStorage, Option<&TaskWorkers>)>,
    q_resources: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            Option<&crate::relationships::StoredIn>,
        ),
        (Without<Designation>, Without<TaskWorkers>),
    >,
    q_stockpiles: Query<&Transform, With<Stockpile>>,
    q_souls: Query<&AssignedTask>,
    q_all_resources: Query<&ResourceItem>,
    q_reserved_items: Query<
        (&ResourceItem, &TargetMixer),
        With<Designation>,
    >,
) {
    // 1. 集計フェーズ: 各ミキサーへの「運搬中」および「予約済み」の数を集計
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();

    for task in q_souls.iter() {
        if let AssignedTask::HaulToMixer(data) = task {
            if let Ok(res_item) = q_all_resources.get(data.item) {
                *in_flight.entry((data.mixer, res_item.0)).or_insert(0) += 1;
            }
        }
        if let AssignedTask::GatherWater(data) = task {
            // GatherWater も MudMixer がターゲットならインフライトとして数える
            // ただし GatherWaterData.tank が Entity なので、それが mixer かどうかで判定
            *in_flight.entry((data.tank, ResourceType::Water)).or_insert(0) += 1;
        }
    }

    // 予約済みアイテムをカウント
    for (res_item, target_mixer) in q_reserved_items.iter() {
        *in_flight.entry((target_mixer.0, res_item.0)).or_insert(0) += 1;
    }

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
                let inflight_count = *in_flight.get(&(mixer_entity, resource_type)).unwrap_or(&0);

                if current + (inflight_count as u32) < MUD_MIXER_CAPACITY {
                    // 近場のアイテムを探す
                    let search_radius = TILE_SIZE * 30.0;
                    let nearby = resource_grid.get_nearby_in_radius(mixer_pos, search_radius);
                    
                    let matching = nearby.into_iter()
                        .filter(|&e| !already_assigned_this_frame.contains(&e))
                        .filter_map(|e| {
                            let Ok((_, transform, vis, res_item, stored_in_opt)) = q_resources.get(e) else { return None; };
                            if *vis == Visibility::Hidden || res_item.0 != resource_type { return None; }
                            
                            // 既に Reserved されているものはクエリの Without<Designation> で除外済み
                            
                            if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                                if let Ok(stock_transform) = q_stockpiles.get(*stock_entity) {
                                    if !task_area.contains(stock_transform.translation.truncate()) { return None; }
                                }
                            }
                            
                            Some((e, transform.translation.truncate().distance_squared(mixer_pos)))
                        })
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                        .map(|a| a.0);

                    if let Some(item_entity) = matching {
                        already_assigned_this_frame.insert(item_entity);
                        *in_flight.entry((mixer_entity, resource_type)).or_insert(0) += 1;

                        commands.entity(item_entity).insert((
                            Designation { work_type: WorkType::Haul },
                            TargetMixer(mixer_entity),
                            TaskSlots::new(1),
                            IssuedBy(_fam_entity),
                        ));
                    }
                }
            }

            // Water の自動リクエスト
            let water_current = storage.water;
            let water_inflight = *in_flight.get(&(mixer_entity, ResourceType::Water)).unwrap_or(&0);
            
            if water_current + (water_inflight as u32) < MUD_MIXER_CAPACITY {
                // 近くの空バケツを探して GatherWater タスクを発行
                let search_radius = TILE_SIZE * 30.0;
                let nearby = resource_grid.get_nearby_in_radius(mixer_pos, search_radius);
                
                let matching_bucket = nearby.into_iter()
                    .filter(|&e| !already_assigned_this_frame.contains(&e))
                    .filter_map(|e| {
                        let Ok((_, transform, vis, res_item, stored_in_opt)) = q_resources.get(e) else { return None; };
                        if *vis == Visibility::Hidden { return None; }
                        // 空バケツのみ（水入りバケツは既に水を持っている）
                        if res_item.0 != ResourceType::BucketEmpty { return None; }
                        
                        if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                            if let Ok(stock_transform) = q_stockpiles.get(*stock_entity) {
                                if !task_area.contains(stock_transform.translation.truncate()) { return None; }
                            }
                        }
                        
                        Some((e, transform.translation.truncate().distance_squared(mixer_pos)))
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .map(|a| a.0);

                if let Some(bucket_entity) = matching_bucket {
                    already_assigned_this_frame.insert(bucket_entity);
                    *in_flight.entry((mixer_entity, ResourceType::Water)).or_insert(0) += 1;

                    commands.entity(bucket_entity).insert((
                        Designation { work_type: WorkType::GatherWater },
                        TargetMixer(mixer_entity),
                        TaskSlots::new(1),
                        IssuedBy(_fam_entity),
                        crate::systems::jobs::Priority(4),
                    ));
                    info!("AUTO_HAUL_MIXER: Issued GatherWater for bucket {:?} to MudMixer {:?}", bucket_entity, mixer_entity);
                }
            }
        }
    }
}
