//! Task reservation lifecycle helpers.
//!
//! `AssignedTask` のフェーズごとに、現在保持すべき予約をここで定義する。
//! - `collect_active_reservation_ops`: Sense 側の再構築で使用
//! - `collect_release_reservation_ops`: 中断時解放で使用

use crate::events::ResourceReservationOp;
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, BuildPhase, CollectBonePhase, CollectSandPhase, GatherPhase, GatherWaterPhase,
    HaulPhase, HaulToBpPhase, HaulToMixerPhase, HaulWaterToMixerPhase, HaulWithWheelbarrowPhase,
    PourFloorPhase, RefinePhase, ReinforceFloorPhase,
};
use bevy::prelude::*;

/// 現在フェーズで保持される予約を `Reserve*` 操作として返す。
pub fn collect_active_reservation_ops(
    task: &AssignedTask,
    mut resolve_wheelbarrow_item_type: impl FnMut(Entity, ResourceType) -> ResourceType,
) -> Vec<ResourceReservationOp> {
    let mut ops = Vec::new();

    match task {
        AssignedTask::Haul(data) => {
            if matches!(data.phase, HaulPhase::GoingToItem) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.item,
                    amount: 1,
                });
            }
        }
        AssignedTask::GatherWater(data) => {
            if matches!(data.phase, GatherWaterPhase::GoingToBucket) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.bucket,
                    amount: 1,
                });
            }
        }
        AssignedTask::HaulWaterToMixer(data) => {
            ops.push(ResourceReservationOp::ReserveMixerDestination {
                target: data.mixer,
                resource_type: ResourceType::Water,
            });

            if matches!(data.phase, HaulWaterToMixerPhase::GoingToBucket) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.bucket,
                    amount: 1,
                });
            }

            if matches!(
                data.phase,
                HaulWaterToMixerPhase::GoingToBucket
                    | HaulWaterToMixerPhase::GoingToTank
                    | HaulWaterToMixerPhase::FillingFromTank
            ) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.tank,
                    amount: 1,
                });
            }
        }
        AssignedTask::HaulToMixer(data) => {
            ops.push(ResourceReservationOp::ReserveMixerDestination {
                target: data.mixer,
                resource_type: data.resource_type,
            });
            if matches!(data.phase, HaulToMixerPhase::GoingToItem) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.item,
                    amount: 1,
                });
            }
        }
        AssignedTask::HaulToBlueprint(data) => {
            if matches!(data.phase, HaulToBpPhase::GoingToItem) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.item,
                    amount: 1,
                });
            }
        }
        AssignedTask::Build(data) => {
            if matches!(
                data.phase,
                BuildPhase::GoingToBlueprint | BuildPhase::Building { .. }
            ) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.blueprint,
                    amount: 1,
                });
            }
        }
        AssignedTask::Gather(data) => {
            if matches!(
                data.phase,
                GatherPhase::GoingToResource | GatherPhase::Collecting { .. }
            ) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.target,
                    amount: 1,
                });
            }
        }
        AssignedTask::CollectSand(data) => {
            if matches!(
                data.phase,
                CollectSandPhase::GoingToSand | CollectSandPhase::Collecting { .. }
            ) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.target,
                    amount: 1,
                });
            }
        }
        AssignedTask::CollectBone(data) => {
            if matches!(
                data.phase,
                CollectBonePhase::GoingToBone | CollectBonePhase::Collecting { .. }
            ) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.target,
                    amount: 1,
                });
            }
        }
        AssignedTask::Refine(data) => {
            if matches!(
                data.phase,
                RefinePhase::GoingToMixer | RefinePhase::Refining { .. }
            ) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.mixer,
                    amount: 1,
                });
            }
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            ops.push(ResourceReservationOp::ReserveSource {
                source: data.wheelbarrow,
                amount: 1,
            });

            for &item in &data.items {
                match data.destination {
                    WheelbarrowDestination::Stockpile(_) | WheelbarrowDestination::Blueprint(_) => {
                        // DeliveringTo リレーションシップで管理するため、ここでは予約不要
                    }
                    WheelbarrowDestination::Mixer {
                        entity: target,
                        resource_type,
                    } => {
                        let item_type = resolve_wheelbarrow_item_type(item, resource_type);
                        ops.push(ResourceReservationOp::ReserveMixerDestination {
                            target,
                            resource_type: item_type,
                        });
                    }
                }
            }

            // items が空でない場合、それぞれのアイテムが DeliveringTo リレーションシップを通じて予約を維持している。
            // 砂の直接採取などの「アイテムがまだ存在しない」場合の予備予約ロジック(destination_reserved)は
            // 現在の設計では DeliveringTo に統一されたため、ここでは items.len() 分の予約のみを考慮する。

            if matches!(
                data.phase,
                HaulWithWheelbarrowPhase::GoingToParking
                    | HaulWithWheelbarrowPhase::PickingUpWheelbarrow
                    | HaulWithWheelbarrowPhase::GoingToSource
            ) || (matches!(data.phase, HaulWithWheelbarrowPhase::Loading)
                && data.collect_source.is_some())
            {
                if let Some(source) = data.collect_source {
                    ops.push(ResourceReservationOp::ReserveSource { source, amount: 1 });
                } else {
                    for &item in &data.items {
                        ops.push(ResourceReservationOp::ReserveSource {
                            source: item,
                            amount: 1,
                        });
                    }
                }
            }
        }
        AssignedTask::ReinforceFloorTile(data) => {
            if !matches!(data.phase, ReinforceFloorPhase::Done) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.tile,
                    amount: 1,
                });
            }
        }
        AssignedTask::PourFloorTile(data) => {
            if !matches!(data.phase, PourFloorPhase::Done) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.tile,
                    amount: 1,
                });
            }
        }
        AssignedTask::None => {}
    }

    ops
}

/// 中断時に解放すべき予約を `Release*` 操作として返す。
pub fn collect_release_reservation_ops(
    task: &AssignedTask,
    resolve_wheelbarrow_item_type: impl FnMut(Entity, ResourceType) -> ResourceType,
) -> Vec<ResourceReservationOp> {
    collect_active_reservation_ops(task, resolve_wheelbarrow_item_type)
        .into_iter()
        .filter_map(to_release_op)
        .collect()
}

fn to_release_op(op: ResourceReservationOp) -> Option<ResourceReservationOp> {
    match op {
        ResourceReservationOp::ReserveMixerDestination {
            target,
            resource_type,
        } => Some(ResourceReservationOp::ReleaseMixerDestination {
            target,
            resource_type,
        }),
        ResourceReservationOp::ReserveSource { source, amount } => {
            Some(ResourceReservationOp::ReleaseSource { source, amount })
        }
        ResourceReservationOp::ReleaseMixerDestination { .. }
        | ResourceReservationOp::ReleaseSource { .. }
        | ResourceReservationOp::RecordPickedSource { .. } => None,
    }
}
