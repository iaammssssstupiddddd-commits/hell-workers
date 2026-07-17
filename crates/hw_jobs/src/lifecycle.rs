//! タスク予約ライフサイクルヘルパー
//!
//! `AssignedTask` のフェーズごとに、現在保持すべき予約をここで定義する。
//! - `collect_active_reservation_ops`: Sense 側の再構築で使用
//! - `collect_release_reservation_ops`: 中断時解放で使用

use bevy::prelude::*;
use hw_core::events::ResourceReservationOp;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};

use crate::tasks::{
    AssignedTask, BuildPhase, CoatWallPhase, CollectBonePhase, FrameWallPhase, GatherPhase,
    HaulPhase, HaulToBpPhase, HaulToMixerPhase, HaulWithWheelbarrowPhase, PourFloorPhase,
    RefinePhase, ReinforceFloorPhase,
};

/// 現在の予約状態を比較するための正規化済みスナップショット。
///
/// `AssignedTask` 自体には progress など `Eq` にできないフィールドがあるため、
/// 実際に予約へ反映する operation だけを保持する。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReservationSignature(Vec<ResourceReservationOp>);

impl ReservationSignature {
    /// `collect_active_reservation_ops` の結果から signature を作る。
    pub fn from_active_ops(ops: Vec<ResourceReservationOp>) -> Self {
        Self(ops)
    }

    /// 予約を持たないタスクかを返す。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// snapshot 再構築時に反映する active operation を返す。
    pub fn active_ops(&self) -> &[ResourceReservationOp] {
        &self.0
    }
}

/// 現在フェーズで保持される予約を `Reserve*` 操作として返す。
pub fn collect_active_reservation_ops(
    task: &AssignedTask,
    mut resolve_wheelbarrow_item_type: impl FnMut(Entity, ResourceType) -> ResourceType,
) -> Vec<ResourceReservationOp> {
    let mut ops = Vec::new();

    if let Some(transport_data) = task.bucket_transport_data() {
        if transport_data.should_reserve_bucket_source() {
            ops.push(ResourceReservationOp::ReserveSource {
                source: transport_data.bucket,
                amount: 1,
            });
        }

        if transport_data.should_reserve_tank_source()
            && let Some(source) = transport_data.tank_source_entity()
        {
            ops.push(ResourceReservationOp::ReserveSource { source, amount: 1 });
        }

        if transport_data.should_reserve_mixer_destination() {
            ops.push(ResourceReservationOp::ReserveMixerDestination {
                target: transport_data.destination_entity(),
                resource_type: ResourceType::Water,
            });
        }
    }

    match task {
        AssignedTask::Haul(data) => {
            if matches!(data.phase, HaulPhase::GoingToItem) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.item,
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
        AssignedTask::MovePlant(_) => {}
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
        AssignedTask::FrameWallTile(data) => {
            if !matches!(data.phase, FrameWallPhase::Done) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.tile,
                    amount: 1,
                });
            }
        }
        AssignedTask::CoatWall(data) => {
            if !matches!(data.phase, CoatWallPhase::Done) {
                ops.push(ResourceReservationOp::ReserveSource {
                    source: data.tile,
                    amount: 1,
                });
            }
        }
        AssignedTask::GeneratePower(data) => {
            ops.push(ResourceReservationOp::ReserveSource {
                source: data.tile,
                amount: 1,
            });
        }
        AssignedTask::BucketTransport(_) | AssignedTask::None => {}
    }

    ops
}

/// 現在フェーズの予約比較用 signature を返す。
///
/// operation の正規化規則を二重に持たないため、必ず
/// `collect_active_reservation_ops` の結果から導出する。
pub fn active_reservation_signature(
    task: &AssignedTask,
    resolve_wheelbarrow_item_type: impl FnMut(Entity, ResourceType) -> ResourceType,
) -> ReservationSignature {
    ReservationSignature::from_active_ops(collect_active_reservation_ops(
        task,
        resolve_wheelbarrow_item_type,
    ))
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

#[cfg(test)]
mod tests;
