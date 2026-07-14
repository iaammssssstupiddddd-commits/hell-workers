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
mod tests {
    use super::*;
    use crate::tasks::*;
    use hw_core::jobs::WorkType;

    fn gathering_task(phase: GatherPhase) -> AssignedTask {
        AssignedTask::Gather(GatherData {
            target: Entity::PLACEHOLDER,
            work_type: WorkType::Chop,
            phase,
        })
    }

    #[test]
    fn reservation_signature_ignores_gather_progress() {
        let first = gathering_task(GatherPhase::Collecting { progress: 0.1 });
        let later = gathering_task(GatherPhase::Collecting { progress: 0.9 });

        assert_eq!(
            active_reservation_signature(&first, |_, fallback| fallback),
            active_reservation_signature(&later, |_, fallback| fallback)
        );
    }

    #[test]
    fn reservation_signature_tracks_reservation_phase_boundaries() {
        let collecting = gathering_task(GatherPhase::Collecting { progress: 0.5 });
        let done = gathering_task(GatherPhase::Done);

        assert_ne!(
            active_reservation_signature(&collecting, |_, fallback| fallback),
            active_reservation_signature(&done, |_, fallback| fallback)
        );
        assert!(active_reservation_signature(&done, |_, fallback| fallback).is_empty());
    }

    #[test]
    fn reservation_signature_is_derived_from_active_operations() {
        let task = gathering_task(GatherPhase::GoingToResource);
        let ops = collect_active_reservation_ops(&task, |_, fallback| fallback);

        assert_eq!(
            ReservationSignature::from_active_ops(ops.clone()),
            active_reservation_signature(&task, |_, fallback| fallback)
        );
    }

    #[test]
    fn generate_power_reserves_its_tile_until_terminal_transition() {
        let tile = Entity::PLACEHOLDER;

        for phase in [
            GeneratePowerPhase::GoingToTile,
            GeneratePowerPhase::Generating,
        ] {
            let task = AssignedTask::GeneratePower(GeneratePowerData {
                tile,
                tile_pos: Vec2::ZERO,
                phase,
            });

            assert_eq!(
                collect_active_reservation_ops(&task, |_, fallback| fallback),
                vec![ResourceReservationOp::ReserveSource {
                    source: tile,
                    amount: 1,
                }]
            );
            assert_eq!(
                collect_release_reservation_ops(&task, |_, fallback| fallback),
                vec![ResourceReservationOp::ReleaseSource {
                    source: tile,
                    amount: 1,
                }]
            );
        }
    }

    #[test]
    fn wheelbarrow_item_source_reservations_end_after_loading() {
        let mut world = World::new();
        let wheelbarrow = world.spawn_empty().id();
        let mixer = world.spawn_empty().id();
        let first_item = world.spawn_empty().id();
        let second_item = world.spawn_empty().id();
        let data = |phase| HaulWithWheelbarrowData {
            wheelbarrow,
            source_pos: Vec2::ZERO,
            destination: WheelbarrowDestination::Mixer {
                entity: mixer,
                resource_type: ResourceType::Wood,
            },
            collect_source: None,
            collect_amount: 0,
            collect_resource_type: None,
            items: vec![first_item, second_item],
            phase,
        };

        let before_loading =
            AssignedTask::HaulWithWheelbarrow(data(HaulWithWheelbarrowPhase::GoingToSource));
        let after_loading =
            AssignedTask::HaulWithWheelbarrow(data(HaulWithWheelbarrowPhase::GoingToDestination));

        assert_eq!(
            collect_active_reservation_ops(&before_loading, |_, fallback| fallback),
            vec![
                ResourceReservationOp::ReserveSource {
                    source: wheelbarrow,
                    amount: 1,
                },
                ResourceReservationOp::ReserveMixerDestination {
                    target: mixer,
                    resource_type: ResourceType::Wood,
                },
                ResourceReservationOp::ReserveMixerDestination {
                    target: mixer,
                    resource_type: ResourceType::Wood,
                },
                ResourceReservationOp::ReserveSource {
                    source: first_item,
                    amount: 1,
                },
                ResourceReservationOp::ReserveSource {
                    source: second_item,
                    amount: 1,
                },
            ]
        );
        assert_eq!(
            collect_active_reservation_ops(&after_loading, |_, fallback| fallback),
            vec![
                ResourceReservationOp::ReserveSource {
                    source: wheelbarrow,
                    amount: 1,
                },
                ResourceReservationOp::ReserveMixerDestination {
                    target: mixer,
                    resource_type: ResourceType::Wood,
                },
                ResourceReservationOp::ReserveMixerDestination {
                    target: mixer,
                    resource_type: ResourceType::Wood,
                },
            ]
        );
    }

    #[test]
    fn every_assigned_task_variant_has_an_explicit_reservation_contract() {
        let mut world = World::new();
        let mut next_entity = || world.spawn_empty().id();
        let bucket = next_entity();
        let tank = next_entity();
        let mixer = next_entity();
        let item = next_entity();
        let target = next_entity();
        let site = next_entity();
        let wall = next_entity();
        let wheelbarrow = next_entity();

        let cases = vec![
            ("none", AssignedTask::None, Vec::new()),
            (
                "haul",
                AssignedTask::Haul(HaulData {
                    item,
                    stockpile: target,
                    phase: HaulPhase::GoingToItem,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: item,
                    amount: 1,
                }],
            ),
            (
                "haul to mixer",
                AssignedTask::HaulToMixer(HaulToMixerData {
                    item,
                    mixer,
                    resource_type: ResourceType::Rock,
                    phase: HaulToMixerPhase::GoingToItem,
                }),
                vec![
                    ResourceReservationOp::ReserveMixerDestination {
                        target: mixer,
                        resource_type: ResourceType::Rock,
                    },
                    ResourceReservationOp::ReserveSource {
                        source: item,
                        amount: 1,
                    },
                ],
            ),
            (
                "haul to blueprint",
                AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                    item,
                    blueprint: target,
                    phase: HaulToBpPhase::GoingToItem,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: item,
                    amount: 1,
                }],
            ),
            (
                "build",
                AssignedTask::Build(BuildData {
                    blueprint: target,
                    phase: BuildPhase::GoingToBlueprint,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
            (
                "move plant",
                AssignedTask::MovePlant(MovePlantData {
                    task_entity: target,
                    building: wall,
                    destination_grid: (0, 0),
                    destination_pos: Vec2::ZERO,
                    companion_anchor: None,
                    phase: MovePlantPhase::GoToBuilding,
                }),
                Vec::new(),
            ),
            (
                "bucket transport",
                AssignedTask::BucketTransport(BucketTransportData {
                    bucket,
                    source: BucketTransportSource::Tank {
                        tank,
                        needs_fill: true,
                    },
                    destination: BucketTransportDestination::Mixer(mixer),
                    amount: 0,
                    phase: BucketTransportPhase::GoingToBucket,
                }),
                vec![
                    ResourceReservationOp::ReserveSource {
                        source: bucket,
                        amount: 1,
                    },
                    ResourceReservationOp::ReserveSource {
                        source: tank,
                        amount: 1,
                    },
                    ResourceReservationOp::ReserveMixerDestination {
                        target: mixer,
                        resource_type: ResourceType::Water,
                    },
                ],
            ),
            (
                "collect bone",
                AssignedTask::CollectBone(CollectBoneData {
                    target,
                    phase: CollectBonePhase::GoingToBone,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
            (
                "gather",
                AssignedTask::Gather(GatherData {
                    target,
                    work_type: WorkType::Chop,
                    phase: GatherPhase::GoingToResource,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
            (
                "refine",
                AssignedTask::Refine(RefineData {
                    mixer,
                    phase: RefinePhase::GoingToMixer,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: mixer,
                    amount: 1,
                }],
            ),
            (
                "wheelbarrow haul",
                AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                    wheelbarrow,
                    source_pos: Vec2::ZERO,
                    destination: WheelbarrowDestination::Stockpile(target),
                    collect_source: None,
                    collect_amount: 0,
                    collect_resource_type: None,
                    items: vec![item],
                    phase: HaulWithWheelbarrowPhase::GoingToSource,
                }),
                vec![
                    ResourceReservationOp::ReserveSource {
                        source: wheelbarrow,
                        amount: 1,
                    },
                    ResourceReservationOp::ReserveSource {
                        source: item,
                        amount: 1,
                    },
                ],
            ),
            (
                "reinforce floor",
                AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
                    tile: target,
                    site,
                    phase: ReinforceFloorPhase::GoingToTile,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
            (
                "pour floor",
                AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: target,
                    site,
                    phase: PourFloorPhase::GoingToTile,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
            (
                "frame wall",
                AssignedTask::FrameWallTile(FrameWallTileData {
                    tile: target,
                    site,
                    phase: FrameWallPhase::GoingToTile,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
            (
                "coat wall",
                AssignedTask::CoatWall(CoatWallData {
                    tile: target,
                    site,
                    wall,
                    phase: CoatWallPhase::GoingToTile,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
            (
                "generate power",
                AssignedTask::GeneratePower(GeneratePowerData {
                    tile: target,
                    tile_pos: Vec2::ZERO,
                    phase: GeneratePowerPhase::GoingToTile,
                }),
                vec![ResourceReservationOp::ReserveSource {
                    source: target,
                    amount: 1,
                }],
            ),
        ];

        for (name, task, expected_active) in cases {
            assert_eq!(
                collect_active_reservation_ops(&task, |_, fallback| fallback),
                expected_active,
                "unexpected active reservation contract for {name}"
            );
        }
    }
}
