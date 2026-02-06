//! Bucket auto-haul system
//!
//! Automatically creates haul tasks for dropped buckets to return them to bucket storage.

use bevy::prelude::*;

use crate::constants::*;
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::{ReservedForTask, ResourceItem, ResourceType, Stockpile};
use crate::systems::soul_ai::work::auto_haul::ItemReservations;
use crate::systems::spatial::{SpatialGridOps, StockpileSpatialGrid};

/// バケツ専用オートホールシステム
/// ドロップされたバケツを、BelongsTo で紐付いたタンクのバケツ置き場に運搬する
pub fn bucket_auto_haul_system(
    mut commands: Commands,
    stockpile_grid: Res<StockpileSpatialGrid>,
    mut item_reservations: ResMut<ItemReservations>,
    q_familiars: Query<(Entity, &TaskArea), With<crate::entities::familiar::Familiar>>,
    q_buckets: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            &crate::systems::logistics::BelongsTo,
            Option<&ReservedForTask>,
            Option<&TaskWorkers>,
        ),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
        ),
    >,
    q_stockpiles: Query<(
        Entity,
        &Transform,
        &Stockpile,
        &crate::systems::logistics::BelongsTo,
        Option<&crate::relationships::StoredItems>,
    )>,
) {
    let mut already_assigned = std::collections::HashSet::new();

    for (fam_entity, task_area) in q_familiars.iter() {
        for (
            bucket_entity,
            bucket_transform,
            visibility,
            res_item,
            bucket_belongs,
            reserved_opt,
            workers_opt,
        ) in q_buckets.iter()
        {
            // バケツ以外はスキップ
            if !matches!(
                res_item.0,
                ResourceType::BucketEmpty | ResourceType::BucketWater
            ) {
                continue;
            }

            // 作業中のバケツはスキップ (TaskWorkersが存在し、かつ空でない場合)
            if let Some(workers) = workers_opt {
                if workers.len() > 0 {
                    continue;
                }
            }

            // 既に割り当て済みならスキップ
            if already_assigned.contains(&bucket_entity) {
                continue;
            }

            // 既にタスク発行済みならスキップ
            if item_reservations.0.contains(&bucket_entity) || reserved_opt.is_some() {
                continue;
            }

            // 非表示ならスキップ
            if *visibility == Visibility::Hidden {
                continue;
            }

            let bucket_pos = bucket_transform.translation.truncate();

            // タスクエリア内にあるかチェック
            if !task_area.contains(bucket_pos) {
                continue;
            }

            // バケツが紐付いているタンク
            let tank_entity = bucket_belongs.0;

            // 近くのストックパイルを空間グリッドで検索
            let search_radius = TILE_SIZE * 20.0;
            let nearby_stockpiles = stockpile_grid.get_nearby_in_radius(bucket_pos, search_radius);

            // 同じタンクに紐付いた、条件に合うストックパイルを探す
            let target_stockpile = nearby_stockpiles
                .iter()
                .filter_map(|&entity| q_stockpiles.get(entity).ok())
                .filter(|(_, _, stock, stock_belongs, stored_opt)| {
                    // 同じタンクに紐付いているか
                    if stock_belongs.0 != tank_entity {
                        return false;
                    }

                    // バケツ置き場（Stockpile）が「バケツ」または「未設定」を受け入れるか確認
                    // 通常、タンクのバケツ置き場は resource_type が None または BucketEmpty/Water
                    let type_match = match stock.resource_type {
                        None => true,
                        Some(t) => {
                            matches!(t, ResourceType::BucketEmpty | ResourceType::BucketWater)
                        }
                    };
                    if !type_match {
                        return false;
                    }

                    // 容量に空きがあるか
                    let current = stored_opt.map(|s| s.len()).unwrap_or(0);
                    current < stock.capacity
                })
                .min_by(|(_, t1, _, _, _), (_, t2, _, _, _)| {
                    let d1 = t1.translation.truncate().distance_squared(bucket_pos);
                    let d2 = t2.translation.truncate().distance_squared(bucket_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(entity, _, _, _, _)| entity);

            if let Some(_stockpile_entity) = target_stockpile {
                already_assigned.insert(bucket_entity);
                item_reservations.0.insert(bucket_entity);
                info!(
                    "BUCKET_HAUL: Issuing Haul task for bucket {:?} to stockpile {:?}",
                    bucket_entity, _stockpile_entity
                );
                commands.entity(bucket_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                    crate::systems::jobs::Priority(5), // バケツ返却は優先度高め
                    ReservedForTask,
                ));
            } else {
                warn!(
                    "BUCKET_HAUL: Found bucket {:?} for tank {:?} but no suitable stockpile found!",
                    bucket_entity, tank_entity
                );
            }
        }
    }
}
