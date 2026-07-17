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
