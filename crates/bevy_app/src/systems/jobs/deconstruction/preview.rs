//! Latest-only hover preflight for the Orders deconstruction tool.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_energy::{PowerConsumer, PowerGenerator, SoulSpaPhase, SoulSpaSite};
use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    AssignedTask, Blueprint, BonePile, BridgeMarker, Building, BuildingType,
    DeconstructionDesignationRejectReason, DeconstructionOrders, DeconstructionPending,
    DeconstructionRejectReason, DeconstructionTargetMarkers, Door, FloorConstructionSite,
    MovePlanned, MovePlantTask, PendingBuildingMove, RestArea, SandPile, WallConstructionSite,
    deconstruction_marker_matches, supports_deconstruction_cleanup,
};
use hw_logistics::ResourceType;
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;
use hw_ui::camera::{MainCamera, world_cursor_pos};

use crate::app_contexts::TaskContext;
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskMode;
use crate::world::map::WorldMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeconstructionHoverStatus {
    Available {
        target: Entity,
        kind: BuildingType,
    },
    Rejected {
        target: Option<Entity>,
        kind: Option<BuildingType>,
        reason: DeconstructionDesignationRejectReason,
    },
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct DeconstructionHoverPreview {
    pub(crate) cursor: Option<Vec2>,
    pub(crate) status: Option<DeconstructionHoverStatus>,
}

type PreviewBuildingStateData<'a> = (
    &'a Building,
    Option<&'a MovePlanned>,
    Option<&'a PendingBuildingMove>,
    Option<&'a DeconstructionPending>,
    Option<&'a DeconstructionOrders>,
);

type PreviewBuildingMarkersData<'a> = (
    Option<&'a Stockpile>,
    Option<&'a MudMixerStorage>,
    Option<&'a RestArea>,
    Option<&'a WheelbarrowParking>,
    Option<&'a SandPile>,
    Option<&'a BonePile>,
    Option<&'a Door>,
    Option<&'a BridgeMarker>,
    Option<&'a SoulSpaSite>,
);

type PreviewBuildingPowerData<'a> = (Option<&'a PowerConsumer>, Option<&'a PowerGenerator>);

type ConstructionRoleFilter = Or<(
    With<Blueprint>,
    With<FloorConstructionSite>,
    With<WallConstructionSite>,
    With<FloorTileBlueprint>,
    With<WallTileBlueprint>,
)>;

#[derive(SystemParam)]
pub(crate) struct DeconstructionPreviewInput<'w, 's> {
    task_context: Res<'w, TaskContext>,
    ui_input_state: Res<'w, UiInputState>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    world_map: Res<'w, WorldMap>,
}

#[derive(SystemParam)]
pub(crate) struct DeconstructionPreviewQueries<'w, 's> {
    q_buildings: Query<'w, 's, PreviewBuildingStateData<'static>>,
    q_markers: Query<'w, 's, PreviewBuildingMarkersData<'static>>,
    q_power: Query<'w, 's, PreviewBuildingPowerData<'static>>,
    q_construction_roles: Query<'w, 's, (), ConstructionRoleFilter>,
    q_move_tasks: Query<'w, 's, &'static MovePlantTask>,
    q_assigned_tasks: Query<'w, 's, &'static AssignedTask>,
}

pub(crate) fn deconstruction_hover_preview_system(
    input: DeconstructionPreviewInput,
    queries: DeconstructionPreviewQueries,
    mut preview: ResMut<DeconstructionHoverPreview>,
) {
    if !matches!(input.task_context.0, TaskMode::DesignateDeconstruct(_))
        || input.ui_input_state.world_input_blocked()
    {
        set_preview(&mut preview, DeconstructionHoverPreview::default());
        return;
    }

    let Ok(window) = input.q_window.single() else {
        set_preview(&mut preview, DeconstructionHoverPreview::default());
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        set_preview(&mut preview, DeconstructionHoverPreview::default());
        return;
    };
    let Some(world_pos) = world_cursor_pos(&input.q_window, &input.q_camera) else {
        set_preview(&mut preview, DeconstructionHoverPreview::default());
        return;
    };
    let grid = WorldMap::world_to_grid(world_pos);
    let hit = input
        .world_map
        .building_entity(grid)
        .or_else(|| input.world_map.floor_entity(grid));
    let status = hit.map_or(
        DeconstructionHoverStatus::Rejected {
            target: None,
            kind: None,
            reason: DeconstructionDesignationRejectReason::NoTarget,
        },
        |hit| {
            classify_hover_target(
                hit,
                &queries.q_buildings,
                &queries.q_markers,
                &queries.q_power,
                &queries.q_construction_roles,
                &queries.q_move_tasks,
                &queries.q_assigned_tasks,
            )
        },
    );

    set_preview(
        &mut preview,
        DeconstructionHoverPreview {
            cursor: Some(cursor),
            status: Some(status),
        },
    );
}

fn classify_hover_target(
    target: Entity,
    q_buildings: &Query<PreviewBuildingStateData<'_>>,
    q_markers: &Query<PreviewBuildingMarkersData<'_>>,
    q_power: &Query<PreviewBuildingPowerData<'_>>,
    q_construction_roles: &Query<(), ConstructionRoleFilter>,
    q_move_tasks: &Query<&MovePlantTask>,
    q_assigned_tasks: &Query<&AssignedTask>,
) -> DeconstructionHoverStatus {
    if q_construction_roles.get(target).is_ok() {
        return rejected_target(
            target,
            None,
            DeconstructionRejectReason::ConstructionInProgress,
        );
    }

    let Ok((building, move_planned, pending_move, pending_deconstruction, orders)) =
        q_buildings.get(target)
    else {
        return rejected_target(target, None, DeconstructionRejectReason::UnsupportedTarget);
    };
    let Ok((stockpile, mixer, rest_area, parking, sand_pile, bone_pile, door, bridge, soul_spa)) =
        q_markers.get(target)
    else {
        return rejected_target(target, None, DeconstructionRejectReason::UnsupportedTarget);
    };
    let Ok((power_consumer, power_generator)) = q_power.get(target) else {
        return rejected_target(target, None, DeconstructionRejectReason::UnsupportedTarget);
    };
    let kind = building.kind;
    if building.is_provisional {
        return rejected_target(
            target,
            Some(kind),
            DeconstructionRejectReason::ConstructionInProgress,
        );
    }
    if !supports_deconstruction_cleanup(kind)
        || !deconstruction_marker_matches(
            kind,
            DeconstructionTargetMarkers {
                water_storage: stockpile
                    .is_some_and(|stockpile| stockpile.resource_type == Some(ResourceType::Water)),
                mud_mixer_storage: mixer.is_some(),
                rest_area: rest_area.is_some(),
                wheelbarrow_parking: parking.is_some(),
                sand_pile: sand_pile.is_some(),
                bone_pile: bone_pile.is_some(),
                door: door.is_some(),
                bridge: bridge.is_some(),
                operational_soul_spa: soul_spa
                    .is_some_and(|site| site.phase == SoulSpaPhase::Operational),
                power_consumer: power_consumer.is_some(),
                power_generator: power_generator.is_some(),
            },
        )
    {
        return DeconstructionHoverStatus::Rejected {
            target: Some(target),
            kind: Some(kind),
            reason: DeconstructionDesignationRejectReason::CleanupUnavailable,
        };
    }

    let external_move = q_move_tasks.iter().any(|task| task.building == target)
        || q_assigned_tasks
            .iter()
            .any(|task| matches!(task, AssignedTask::MovePlant(data) if data.building == target));
    if move_planned.is_some() || pending_move.is_some() || external_move {
        return rejected_target(target, Some(kind), DeconstructionRejectReason::Moving);
    }
    if pending_deconstruction.is_some() || orders.is_some_and(|orders| !orders.is_empty()) {
        return rejected_target(
            target,
            Some(kind),
            DeconstructionRejectReason::AlreadyDesignated,
        );
    }

    DeconstructionHoverStatus::Available { target, kind }
}

const fn rejected_target(
    target: Entity,
    kind: Option<BuildingType>,
    reason: DeconstructionRejectReason,
) -> DeconstructionHoverStatus {
    DeconstructionHoverStatus::Rejected {
        target: Some(target),
        kind,
        reason: DeconstructionDesignationRejectReason::Target(reason),
    }
}

fn set_preview(preview: &mut DeconstructionHoverPreview, next: DeconstructionHoverPreview) {
    if *preview != next {
        *preview = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_target_keeps_the_canonical_entity_and_typed_reason() {
        let target = Entity::from_raw_u32(3).expect("fixture entity");
        assert_eq!(
            rejected_target(
                target,
                Some(BuildingType::Tank),
                DeconstructionRejectReason::Moving,
            ),
            DeconstructionHoverStatus::Rejected {
                target: Some(target),
                kind: Some(BuildingType::Tank),
                reason: DeconstructionDesignationRejectReason::Target(
                    DeconstructionRejectReason::Moving,
                ),
            }
        );
    }
}
