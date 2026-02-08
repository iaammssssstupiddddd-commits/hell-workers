//! Task area auto-haul system
//!
//! Automatically creates haul tasks for resources within the task area.

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::familiar::ActiveCommand;
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::{
    BelongsTo, ReservedForTask, ResourceItem, ResourceType, Stockpile,
};
use crate::systems::soul_ai::work::auto_haul::ItemReservations;
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps, StockpileSpatialGrid};

/// 指揮エリア内での自動運搬タスク生成システム
///
/// 汎用アイテムは空間検索で、専用アイテム（所有権あり）はRelationshipで検索します。
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
    stockpile_grid: Res<StockpileSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&StoredItems>,
        Option<&BelongsTo>,
    )>,
    q_resources: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            Option<&BelongsTo>,
        ),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::jobs::TargetBlueprint>,
        ),
    >,
    mut item_reservations: ResMut<ItemReservations>,
) {
    let mut already_assigned = std::collections::HashSet::new();

    // -----------------------------------------------------------------------
    // 1. 専用アイテム（所有権あり）の回収
    // 空間に関係なく、所有権の一致するストックパイルへ戻す
    // -----------------------------------------------------------------------
    for (item_entity, _transform, visibility, res_item, item_belongs_opt) in q_resources.iter() {
        if *visibility == Visibility::Hidden {
            continue;
        }
        if item_reservations.0.contains(&item_entity) {
            continue;
        }
        if already_assigned.contains(&item_entity) {
            continue;
        }

        // 所有権がないアイテムはここでは扱わない（後半の空間検索で扱う）
        let Some(item_owner) = item_belongs_opt else {
            continue;
        };

        // このアイテムを受け入れるストックパイルを探す
        // Note: 単純化のため、最初に見つかった適切なストックパイルに割り当てる
        for (_stock_entity, _stock_transform, stockpile, stored, stock_belongs_opt) in
            q_stockpiles.iter()
        {
            // ストックパイルも同じ所有者でなければならない
            if stock_belongs_opt.map(|b| b.0) != Some(item_owner.0) {
                continue;
            }

            // 容量チェック
            if stored.map(|s| s.len()).unwrap_or(0) >= stockpile.capacity {
                continue;
            }

            // 型チェック（バケツ特例含む）
            if let Some(target_type) = stockpile.resource_type {
                let is_bucket_target = matches!(
                    target_type,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                );
                let is_bucket_item = matches!(
                    res_item.0,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                );

                if is_bucket_target && is_bucket_item {
                    // バケツ同士ならOK
                } else if res_item.0 != target_type {
                    continue;
                }
            }

            // タスク発行（発行者はそのアイテムの持ち主であるFamiliarを探すべきだが、
            // 簡易的に TaskArea を持っている Familiar を使うか、あるいはランダムに選ぶ）
            // バケツはタンクに属しており、タンクはエリア内にあるはず。
            // ここでは「エリア内にあるFamiliar」を代表として使う。

            if let Some((fam_entity, _, _)) = q_familiars.iter().next() {
                already_assigned.insert(item_entity);
                item_reservations.0.insert(item_entity);
                commands.entity(item_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                    crate::systems::jobs::Priority(10), // 専用品の回収は最優先
                    ReservedForTask,
                ));
                break; // 1つ割り当てたら次のアイテムへ
            }
        }
    }

    // -----------------------------------------------------------------------
    // 2. 汎用アイテム（所有権なし）の回収
    // エリア内のストックパイルを中心に、近くのアイテムを空間検索で探す
    // -----------------------------------------------------------------------
    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        // タスクエリア内のストックパイルのみを取得
        let stockpiles_in_area = stockpile_grid.get_in_area(task_area.min, task_area.max);

        for stock_entity in stockpiles_in_area {
            let Ok((_stock_entity, stock_transform, stockpile, stored_items_opt, stock_belongs)) =
                q_stockpiles.get(stock_entity)
            else {
                continue;
            };

            // 専用ストックパイル（所有権あり）は、汎用アイテムを受け入れない
            if stock_belongs.is_some() {
                continue;
            }

            let stock_pos = stock_transform.translation.truncate();

            let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
            if current_count >= stockpile.capacity {
                continue;
            }

            let search_radius = TILE_SIZE * 15.0;
            let nearby_resources = resource_grid.get_nearby_in_radius(stock_pos, search_radius);

            let nearest_resource = nearby_resources
                .iter()
                .filter(|&&entity| !already_assigned.contains(&entity))
                .filter(|&&entity| !item_reservations.0.contains(&entity))
                .filter_map(|&entity| {
                    let Ok((_, transform, visibility, res_item, item_belongs)) =
                        q_resources.get(entity)
                    else {
                        return None;
                    };
                    if *visibility == Visibility::Hidden {
                        return None;
                    }

                    // 所有権があるアイテムは除外（前半のループで処理済みのはずだが念のため）
                    if item_belongs.is_some() {
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
                item_reservations.0.insert(item_entity);
                commands.entity(item_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                    crate::systems::jobs::Priority(0),
                    ReservedForTask,
                ));
            }
        }
    }
}
