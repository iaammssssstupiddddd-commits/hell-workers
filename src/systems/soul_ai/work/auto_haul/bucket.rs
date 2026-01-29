//! Bucket auto-haul system
//!
//! Automatically creates haul tasks for dropped buckets to return them to bucket storage.

use bevy::prelude::*;

use crate::entities::familiar::ActiveCommand;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::{ResourceItem, ResourceType, Stockpile};
use crate::relationships::TaskWorkers;

/// バケツ専用オートホールシステム
/// ドロップされたバケツを、BelongsTo で紐付いたタンクのバケツ置き場に運搬する
pub fn bucket_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_buckets: Query<
        (Entity, &Transform, &Visibility, &ResourceItem, &crate::systems::logistics::BelongsTo),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
            Without<TaskWorkers>,
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

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        for (bucket_entity, bucket_transform, visibility, res_item, bucket_belongs) in q_buckets.iter() {
            // バケツ以外はスキップ
            if !matches!(res_item.0, ResourceType::BucketEmpty | ResourceType::BucketWater) {
                continue;
            }

            // 既に割り当て済みならスキップ
            if already_assigned.contains(&bucket_entity) {
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

            // 同じタンクに紐付いたストックパイルを探す
            let target_stockpile = q_stockpiles
                .iter()
                .filter(|(_, _, stock, stock_belongs, stored_opt)| {
                    // 同じタンクに紐付いているか
                    if stock_belongs.0 != tank_entity {
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
                commands.entity(bucket_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                    crate::systems::jobs::Priority(5), // バケツ返却は優先度高め
                ));
            }
        }
    }
}
