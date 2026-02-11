use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState, Path};
use crate::events::{ResourceReservationOp, ResourceReservationRequest};
use crate::relationships::WorkingOn;
// use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache; // Removed unused import
use crate::systems::logistics::{Inventory, ResourceType};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::execute::task_execution::context::TaskReservationAccess;
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// 魂が作業可能な状態（待機中かつ健康）であるかを確認する
pub fn is_soul_available_for_work(
    soul: &DamnedSoul,
    task: &AssignedTask,
    idle: &IdleState,
    has_breakdown: bool,
    fatigue_threshold: f32,
) -> bool {
    if !matches!(*task, AssignedTask::None) {
        return false;
    }
    if idle.behavior == IdleBehavior::ExhaustedGathering {
        return false;
    }
    if soul.fatigue >= fatigue_threshold {
        return false;
    }
    if has_breakdown {
        return false;
    }
    true
}

/// 魂からタスクの割り当てを解除し、スロットを解放する。
///
/// ソウル側のみを処理し、タスク側（Designation, IssuedBy）には触らない。
/// 使い魔がスロットの空きを検知して別のソウルに再アサインする。
pub fn unassign_task<'w, 's, Q: TaskReservationAccess<'w, 's>>(
    commands: &mut Commands,
    soul_entity: Entity,
    drop_pos: Vec2,
    task: &mut AssignedTask,
    path: &mut Path,
    mut inventory: Option<&mut Inventory>,
    dropped_item_res: Option<ResourceType>,
    queries: &mut Q,
    world_map: &WorldMap,
    emit_abandoned_event: bool,
) {
    // タスク中断イベントを発火
    if !matches!(*task, AssignedTask::None) && emit_abandoned_event {
        commands.trigger(crate::events::OnTaskAbandoned {
            entity: soul_entity,
        });
    }

    let (gx, gy) = WorldMap::world_to_grid(drop_pos);
    let drop_grid = if world_map.is_walkable(gx, gy) {
        (gx, gy)
    } else {
        world_map
            .get_nearest_walkable_grid(drop_pos)
            .unwrap_or((gx, gy))
    };
    let snapped_pos = WorldMap::grid_to_world(drop_grid.0, drop_grid.1);

    let mut skip_inventory_drop_for: Option<Entity> = None;

    // 運搬・水汲みタスクの予約を解除
    match task {
        AssignedTask::Haul(data) => {
            queries
                .reservation_writer()
                .write(ResourceReservationRequest {
                    op: ResourceReservationOp::ReleaseDestination {
                        target: data.stockpile,
                    },
                });
            use crate::systems::soul_ai::execute::task_execution::types::HaulPhase;
            if matches!(data.phase, HaulPhase::GoingToItem) {
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseSource {
                            source: data.item,
                            amount: 1,
                        },
                    });
            }
        }
        AssignedTask::GatherWater(data) => {
            queries
                .reservation_writer()
                .write(ResourceReservationRequest {
                    op: ResourceReservationOp::ReleaseDestination { target: data.tank },
                });
            use crate::systems::soul_ai::execute::task_execution::types::GatherWaterPhase;
            if matches!(data.phase, GatherWaterPhase::GoingToBucket) {
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseSource {
                            source: data.bucket,
                            amount: 1,
                        },
                    });
            }
        }
        AssignedTask::HaulWaterToMixer(data) => {
            // 作業員スロットとしてのMixer予約解除
            queries
                .reservation_writer()
                .write(ResourceReservationRequest {
                    op: ResourceReservationOp::ReleaseMixerDestination {
                        target: data.mixer,
                        resource_type: ResourceType::Water,
                    },
                });

            use crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerPhase;
            if matches!(data.phase, HaulWaterToMixerPhase::GoingToBucket) {
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseSource {
                            source: data.bucket,
                            amount: 1,
                        },
                    });
            }
            // タンク取水ロックは「実際に取水完了するまで」保持する
            if matches!(
                data.phase,
                HaulWaterToMixerPhase::GoingToBucket
                    | HaulWaterToMixerPhase::GoingToTank
                    | HaulWaterToMixerPhase::FillingFromTank
            ) {
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseSource {
                            source: data.tank,
                            amount: 1,
                        },
                    });
            }
        }
        AssignedTask::HaulToMixer(data) => {
            queries
                .reservation_writer()
                .write(ResourceReservationRequest {
                    op: ResourceReservationOp::ReleaseMixerDestination {
                        target: data.mixer,
                        resource_type: data.resource_type,
                    },
                });

            use crate::systems::soul_ai::execute::task_execution::types::HaulToMixerPhase;
            if matches!(data.phase, HaulToMixerPhase::GoingToItem) {
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseSource {
                            source: data.item,
                            amount: 1,
                        },
                    });
            }
        }
        AssignedTask::HaulToBlueprint(data) => {
            queries
                .reservation_writer()
                .write(ResourceReservationRequest {
                    op: ResourceReservationOp::ReleaseDestination {
                        target: data.blueprint,
                    },
                });

            use crate::systems::soul_ai::execute::task_execution::types::HaulToBpPhase;
            if matches!(data.phase, HaulToBpPhase::GoingToItem) {
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseSource {
                            source: data.item,
                            amount: 1,
                        },
                    });
            }
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            // ソースアイテムの予約と目的地の予約を解放（各アイテムにつき1つずつ）
            for &item in &data.items {
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseSource {
                            source: item,
                            amount: 1,
                        },
                    });
                queries
                    .reservation_writer()
                    .write(ResourceReservationRequest {
                        op: ResourceReservationOp::ReleaseDestination {
                            target: data.dest_stockpile,
                        },
                    });
            }
            // 積載中のアイテムを地面に落とす
            for &item_entity in &data.items {
                if let Ok(mut entity_commands) = commands.get_entity(item_entity) {
                    entity_commands.remove::<crate::relationships::LoadedIn>();
                    entity_commands.insert((
                        Visibility::Visible,
                        Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
                    ));
                }
            }
            // 手押し車を駐車状態に戻す
            if let Ok(mut wb_commands) = commands.get_entity(data.wheelbarrow) {
                wb_commands.remove::<(crate::relationships::PushedBy, crate::systems::visual::haul::WheelbarrowMovement)>();
                if let Some(parking_entity) = queries.belongs_to(data.wheelbarrow) {
                    wb_commands.insert(crate::relationships::ParkedAt(parking_entity));
                }
                wb_commands.insert((
                    Visibility::Visible,
                    Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
                ));
            }
            skip_inventory_drop_for = Some(data.wheelbarrow);
        }
        AssignedTask::CollectSand(_) | AssignedTask::Refine(_) => {}
        _ => {}
    }

    // アイテムのドロップ処理（運搬タスクの場合）
    if let Some(inventory) = inventory.as_deref_mut() {
        if let Some(item_entity) = inventory.0 {
            if Some(item_entity) != skip_inventory_drop_for {
                // インベントリから削除
                // 注意: inventoryは可変参照なので直接書き換える
                // しかし、呼び出し元でinventory.0 = Noneする必要があるため、ここではドロップ処理のみ行う
                // あるいはここでinventory.0 = Noneする?
                // argument is Option<&mut Inventory>.

                // クリーンな状態でドロップ（Designation なし）
                commands.entity(item_entity).insert((
                    Visibility::Visible,
                    Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
                ));

                let _res_item = dropped_item_res
                    .or_else(|| queries.resources().get(item_entity).ok().map(|r| r.0));

                // 管理コンポーネントは削除せず維持する...
                commands
                    .entity(item_entity)
                    .remove::<crate::systems::jobs::TargetBlueprint>();
                commands
                    .entity(item_entity)
                    .remove::<crate::systems::jobs::Priority>();

                // TaskWorkersも確実に削除してリセットする
                commands
                    .entity(item_entity)
                    .remove::<crate::relationships::TaskWorkers>();

                // StoredIn関係は削除（地面に落ちるため）
                commands
                    .entity(item_entity)
                    .remove::<crate::relationships::StoredIn>();
                // ストックパイル情報も削除（地面に落ちるため、確実に非備蓄状態にする）
                commands
                    .entity(item_entity)
                    .remove::<crate::systems::logistics::InStockpile>();

                // Note: ここで即座に新しいタスク(Designation)を付与しない。
                // オートホールシステム(task_area_auto_haul_system)に回収を任せることで、
                // 状況に応じた適切なタスク(Haul)が発行されるようにする。
            }
        }
        inventory.0 = None;
    }

    // ソウルからタスクを解除
    commands.entity(soul_entity).remove::<WorkingOn>();

    *task = AssignedTask::None;
    path.waypoints.clear();
}
