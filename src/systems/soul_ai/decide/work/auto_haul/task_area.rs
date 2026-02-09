//! Task area auto-haul system
//!
//! Automatically creates haul tasks for resources within the task area.

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::events::{DesignationOp, DesignationRequest};
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, WorkType};
use crate::systems::logistics::{
    BelongsTo, ResourceItem, ResourceType, Stockpile,
};
use crate::systems::soul_ai::decide::work::auto_haul::ItemReservations;
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps, StockpileSpatialGrid};

/// 指揮エリア内での自動運搬タスク生成システム
///
/// 汎用アイテムは空間検索で、専用アイテム（所有権あり）はRelationshipで検索します。
pub fn task_area_auto_haul_system(
    mut designation_writer: MessageWriter<DesignationRequest>,
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

            // タスク発行:
            // ランダムな Familiar に紐付けると、割り当て時に TaskArea 制約で
            // 受け入れ先ストックパイルが見えず、未割り当て化しやすい。
            // 受け入れ先ストックパイルを含む TaskArea の Familiar に発行する。
            let stock_pos = _stock_transform.translation.truncate();
            let issuing_familiar = q_familiars
                .iter()
                .filter(|(_, active_command, _)| {
                    !matches!(active_command.command, FamiliarCommand::Idle)
                })
                .find(|(_, _, area)| area.contains(stock_pos))
                .map(|(fam_entity, _, _)| fam_entity)
                .or_else(|| {
                    // どのエリアにも含まれない場合は、最も近い Familiar をフォールバックで使用。
                    q_familiars
                        .iter()
                        .filter(|(_, active_command, _)| {
                            !matches!(active_command.command, FamiliarCommand::Idle)
                        })
                        .min_by(|(_, _, area1), (_, _, area2)| {
                            let d1 = area1.center().distance_squared(stock_pos);
                            let d2 = area2.center().distance_squared(stock_pos);
                            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(fam_entity, _, _)| fam_entity)
                });

            if let Some(fam_entity) = issuing_familiar {
                already_assigned.insert(item_entity);
                item_reservations.0.insert(item_entity);
                designation_writer.write(DesignationRequest {
                    entity: item_entity,
                    operation: DesignationOp::Issue {
                        work_type: WorkType::Haul,
                        issued_by: fam_entity,
                        task_slots: 1,
                        priority: Some(10), // 専用品の回収は最優先
                        target_blueprint: None,
                        target_mixer: None,
                        reserved_for_task: true,
                    },
                });
                break; // 1つ割り当てたら次のアイテムへ
            }
        }
    }

    // -----------------------------------------------------------------------
    // 2. 汎用アイテム（所有権なし）の回収
    // エリア内のストックパイルを中心に、近くのアイテムを空間検索で探す
    // -----------------------------------------------------------------------
    for (fam_entity, active_command, task_area) in q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

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
                designation_writer.write(DesignationRequest {
                    entity: item_entity,
                    operation: DesignationOp::Issue {
                        work_type: WorkType::Haul,
                        issued_by: fam_entity,
                        task_slots: 1,
                        priority: Some(0),
                        target_blueprint: None,
                        target_mixer: None,
                        reserved_for_task: true,
                    },
                });
            }
        }
    }
}
