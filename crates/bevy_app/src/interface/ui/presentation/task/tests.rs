use super::*;
use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};
use hw_jobs::*;

#[test]
fn task_kind_presentation_is_exhaustive_test() {
    let e = Entity::PLACEHOLDER;
    let cases = vec![
        (AssignedTask::None, TaskVisual::Idle, "Idle", None),
        (
            AssignedTask::Gather(GatherData {
                target: e,
                work_type: WorkType::Chop,
                phase: default(),
            }),
            TaskVisual::Chop,
            "Gather",
            Some("GoingToResource"),
        ),
        (
            AssignedTask::Gather(GatherData {
                target: e,
                work_type: WorkType::Mine,
                phase: default(),
            }),
            TaskVisual::Mine,
            "Gather",
            Some("GoingToResource"),
        ),
        (
            AssignedTask::Gather(GatherData {
                target: e,
                work_type: WorkType::GatherWater,
                phase: default(),
            }),
            TaskVisual::GatherDefault,
            "Gather",
            Some("GoingToResource"),
        ),
        (
            AssignedTask::Haul(HaulData {
                item: e,
                stockpile: e,
                phase: default(),
            }),
            TaskVisual::Haul,
            "Haul",
            Some("GoingToItem"),
        ),
        (
            AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                item: e,
                blueprint: e,
                phase: default(),
            }),
            TaskVisual::HaulToBlueprint,
            "HaulToBp",
            Some("GoingToItem"),
        ),
        (
            AssignedTask::Build(BuildData {
                blueprint: e,
                phase: default(),
            }),
            TaskVisual::Build,
            "Build",
            Some("GoingToBlueprint"),
        ),
        (
            AssignedTask::MovePlant(MovePlantData {
                task_entity: e,
                building: e,
                destination_grid: (0, 0),
                destination_pos: Vec2::ZERO,
                companion_anchor: None,
                phase: default(),
            }),
            TaskVisual::Move,
            "MovePlant",
            Some("GoToBuilding"),
        ),
        (
            AssignedTask::CollectBone(CollectBoneData {
                target: e,
                phase: default(),
            }),
            TaskVisual::CollectBone,
            "CollectBone",
            Some("GoingToBone"),
        ),
        (
            AssignedTask::Refine(RefineData {
                mixer: e,
                phase: default(),
            }),
            TaskVisual::Refine,
            "Refine",
            Some("GoingToMixer"),
        ),
        (
            AssignedTask::HaulToMixer(HaulToMixerData {
                item: e,
                mixer: e,
                resource_type: ResourceType::Rock,
                phase: default(),
            }),
            TaskVisual::HaulToBlueprint,
            "HaulToMixer",
            Some("GoingToItem"),
        ),
        (
            AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                wheelbarrow: e,
                source_pos: Vec2::ZERO,
                destination: WheelbarrowDestination::Stockpile(e),
                collect_source: None,
                collect_amount: 0,
                collect_resource_type: None,
                items: Vec::new(),
                phase: default(),
            }),
            TaskVisual::Haul,
            "HaulWheelbarrow",
            Some("GoingToParking"),
        ),
        (
            AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
                tile: e,
                site: e,
                phase: default(),
            }),
            TaskVisual::Build,
            "ReinforceFloor",
            Some("GoingToMaterialCenter"),
        ),
        (
            AssignedTask::PourFloorTile(PourFloorTileData {
                tile: e,
                site: e,
                phase: default(),
            }),
            TaskVisual::Build,
            "PourFloor",
            Some("GoingToMaterialCenter"),
        ),
        (
            AssignedTask::FrameWallTile(FrameWallTileData {
                tile: e,
                site: e,
                phase: default(),
            }),
            TaskVisual::Build,
            "FrameWall",
            Some("GoingToMaterialCenter"),
        ),
        (
            AssignedTask::CoatWall(CoatWallData {
                tile: e,
                site: e,
                wall: e,
                phase: default(),
            }),
            TaskVisual::Build,
            "CoatWall",
            Some("GoingToMaterialCenter"),
        ),
        (
            AssignedTask::GeneratePower(GeneratePowerData {
                tile: e,
                tile_pos: Vec2::ZERO,
                phase: default(),
            }),
            TaskVisual::GeneratePower,
            "GeneratePower",
            Some("GoingToTile"),
        ),
        (
            AssignedTask::Deconstruct(DeconstructData {
                order: e,
                target: e,
                phase: default(),
            }),
            TaskVisual::Deconstruct,
            "Deconstruct",
            Some("GoingToTarget"),
        ),
    ];
    for (task, visual, name, phase) in cases {
        let presentation = task_kind_presentation(&task);
        assert_eq!(presentation.list_visual, visual);
        assert_eq!(presentation.detail_name, name);
        assert_eq!(format_task_phase(&task).as_deref(), phase);
        let expected = phase.map_or_else(|| name.to_owned(), |phase| format!("{name} ({phase})"));
        assert_eq!(format_task_str(&task), expected);
    }
    for source in [
        BucketTransportSource::River,
        BucketTransportSource::Tank {
            tank: e,
            needs_fill: true,
        },
    ] {
        let task = AssignedTask::BucketTransport(BucketTransportData {
            bucket: e,
            source,
            destination: BucketTransportDestination::Mixer(e),
            amount: 0,
            phase: default(),
        });
        assert_eq!(task_kind_presentation(&task).list_visual, TaskVisual::Water);
        assert_eq!(format_task_str(&task), "BucketTransport (GoingToBucket)");
    }
}

#[test]
fn task_phase_and_detail_are_preserved_test() {
    let e = Entity::PLACEHOLDER;
    let power = AssignedTask::GeneratePower(GeneratePowerData {
        tile: e,
        tile_pos: Vec2::ZERO,
        phase: GeneratePowerPhase::Generating,
    });
    assert_eq!(format_task_str(&power), "GeneratePower (Generating)");
    for (phase, expected) in [
        (
            DeconstructPhase::GoingToTarget,
            "Deconstruct (GoingToTarget)",
        ),
        (
            DeconstructPhase::Dismantling { progress: 0.25 },
            "Deconstruct (Dismantling { progress: 0.25 })",
        ),
        (
            DeconstructPhase::AwaitingCommit,
            "Deconstruct (AwaitingCommit)",
        ),
    ] {
        assert_eq!(
            format_task_str(&AssignedTask::Deconstruct(DeconstructData {
                order: e,
                target: e,
                phase
            })),
            expected
        );
    }
}
