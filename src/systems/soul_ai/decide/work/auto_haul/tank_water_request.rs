//! Tank water request system
//!
//! Monitors tank storage levels and issues water gathering tasks when tanks are low.

use crate::constants::BUCKET_CAPACITY;
use bevy::prelude::*;

use crate::relationships::{StoredIn, StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::resource_cache::SharedResourceCache;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::{
    BelongsTo, ReservedForTask, ResourceItem, ResourceType, Stockpile,
};

/// タンクの貯蔵量を監視し、空きがあればバケツに水汲み指示を出すシステム
pub fn tank_water_request_system(
    mut commands: Commands,
    haul_cache: Res<SharedResourceCache>,
    q_familiars: Query<(Entity, &TaskArea)>,
    // タンク自体の在庫状況（Water を貯める Stockpile）
    q_tanks: Query<(Entity, &Transform, &Stockpile, Option<&StoredItems>)>,
    // 全てのバケツ
    q_buckets: Query<(
        Entity,
        &ResourceItem,
        &BelongsTo,
        &Visibility,
        Option<&ReservedForTask>,
        Option<&StoredIn>,
        Option<&Designation>,
        Option<&TaskWorkers>,
    )>,
    mut item_reservations: ResMut<crate::systems::soul_ai::work::auto_haul::ItemReservations>,
) {
    for (tank_entity, tank_transform, tank_stock, stored_opt) in q_tanks.iter() {
        // 水タンク以外はスキップ
        if tank_stock.resource_type != Some(ResourceType::Water) {
            continue;
        }

        let current_water = stored_opt.map(|s| s.len()).unwrap_or(0);
        let reserved_water_tasks = haul_cache.get_destination_reservation(tank_entity);
        let total_water = (current_water as u32) + (reserved_water_tasks as u32 * BUCKET_CAPACITY);

        if total_water < tank_stock.capacity as u32 {
            let needed_water = tank_stock.capacity as u32 - total_water;
            let needed_tasks = (needed_water + BUCKET_CAPACITY - 1) / BUCKET_CAPACITY;
            let mut issued = 0;

            // このタンクに紐付いたバケツを探す
            for (
                bucket_entity,
                res_item,
                bucket_belongs,
                visibility,
                reserved_opt,
                _stored_in,
                designation,
                workers,
            ) in q_buckets.iter()
            {
                if issued >= needed_tasks {
                    break;
                }

                if *visibility == Visibility::Hidden {
                    continue;
                }

                if reserved_opt.is_some() || item_reservations.0.contains(&bucket_entity) {
                    continue;
                }

                if workers.is_some() {
                    continue;
                }

                // 既にタスクが付与されているバケツはスキップ（上書き防止）
                if designation.is_some() {
                    continue;
                }

                if bucket_belongs.0 != tank_entity {
                    continue;
                }

                // バケツ（空または水入り）であることを確認
                if !matches!(
                    res_item.0,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                ) {
                    continue;
                }

                // このバケツを管理しているファミリアを探す（タスクエリアに基づく）
                let tank_pos = tank_transform.translation.truncate();
                let issued_by = q_familiars
                    .iter()
                    .filter(|(_, area)| area.contains(tank_pos))
                    .map(|(fam, _)| fam)
                    .next();

                if let Some(fam_entity) = issued_by {
                    item_reservations.0.insert(bucket_entity);
                    commands.entity(bucket_entity).insert((
                        Designation {
                            work_type: WorkType::GatherWater,
                        },
                        IssuedBy(fam_entity),
                        TaskSlots::new(1),
                        crate::systems::jobs::Priority(3),
                        ReservedForTask,
                    ));
                    issued += 1;
                    info!(
                        "TANK_WATCH: Issued GatherWater for bucket {:?} (Tank {:?})",
                        bucket_entity, tank_entity
                    );
                }
            }
        }
    }
}
