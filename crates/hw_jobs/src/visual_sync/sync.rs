//! 定期同期 System — コンポーネントの変化を検知して visual mirror コンポーネントを更新する。

use bevy::prelude::*;

use hw_core::jobs::WorkType;
use hw_core::visual_mirror::building::{BuildingVisualState, MudMixerVisualState};
use hw_core::visual_mirror::construction::{
    BlueprintVisualState, FloorConstructionPhaseMirror, FloorSiteVisualState, FloorTileStateMirror,
    FloorTileVisualMirror, WallSiteVisualState, WallTileStateMirror, WallTileVisualMirror,
};
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};

use crate::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use crate::model::{Blueprint, Building, BuildingType};
use crate::tasks::{
    AssignedTask, CoatWallPhase, FrameWallPhase, GatherPhase, HaulPhase, PourFloorPhase,
    RefinePhase, ReinforceFloorPhase,
};

use super::building_type_to_visual;

type SoulTaskSyncQuery<'w, 's> = Query<
    'w,
    's,
    (&'static AssignedTask, &'static mut SoulTaskVisualState),
    Or<(Changed<AssignedTask>, Added<AssignedTask>)>,
>;
type BlueprintSyncQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Blueprint, &'static mut BlueprintVisualState),
    Or<(Changed<Blueprint>, Added<Blueprint>)>,
>;
type FloorTileSyncQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static FloorTileBlueprint,
        &'static mut FloorTileVisualMirror,
    ),
    Or<(Changed<FloorTileBlueprint>, Added<FloorTileBlueprint>)>,
>;
type WallTileSyncQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static WallTileBlueprint,
        &'static mut WallTileVisualMirror,
    ),
    Or<(Changed<WallTileBlueprint>, Added<WallTileBlueprint>)>,
>;
type FloorSiteSyncQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static FloorConstructionSite,
        &'static mut FloorSiteVisualState,
    ),
    Or<(Changed<FloorConstructionSite>, Added<FloorConstructionSite>)>,
>;
type WallSiteSyncQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static WallConstructionSite,
        &'static mut WallSiteVisualState,
    ),
    Or<(Changed<WallConstructionSite>, Added<WallConstructionSite>)>,
>;

pub fn sync_soul_task_visual_system(mut q: SoulTaskSyncQuery) {
    for (task, mut state) in q.iter_mut() {
        let (phase, progress, link_target, bucket_link) = match task {
            AssignedTask::None => (SoulTaskPhaseVisual::None, None, None, None),
            AssignedTask::Gather(d) => {
                let phase = match d.work_type {
                    WorkType::Chop => SoulTaskPhaseVisual::GatherChop,
                    WorkType::Mine => SoulTaskPhaseVisual::GatherMine,
                    _ => SoulTaskPhaseVisual::GatherChop,
                };
                let progress = if let GatherPhase::Collecting { progress } = d.phase {
                    Some(progress)
                } else {
                    None
                };
                (phase, progress, Some(d.target), None)
            }
            AssignedTask::Haul(d) => {
                let target = match d.phase {
                    HaulPhase::GoingToItem => Some(d.item),
                    HaulPhase::GoingToStockpile => Some(d.stockpile),
                    _ => None,
                };
                (SoulTaskPhaseVisual::Haul, None, target, None)
            }
            AssignedTask::HaulToBlueprint(d) => (
                SoulTaskPhaseVisual::HaulToBlueprint,
                None,
                Some(d.blueprint),
                None,
            ),
            AssignedTask::Build(d) => (SoulTaskPhaseVisual::Build, None, Some(d.blueprint), None),
            AssignedTask::ReinforceFloorTile(d) => {
                let progress = if let ReinforceFloorPhase::Reinforcing { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (
                    SoulTaskPhaseVisual::ReinforceFloor,
                    progress,
                    Some(d.tile),
                    None,
                )
            }
            AssignedTask::PourFloorTile(d) => {
                let progress = if let PourFloorPhase::Pouring { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (SoulTaskPhaseVisual::PourFloor, progress, Some(d.tile), None)
            }
            AssignedTask::FrameWallTile(d) => {
                let progress = if let FrameWallPhase::Framing { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (SoulTaskPhaseVisual::FrameWall, progress, Some(d.tile), None)
            }
            AssignedTask::CoatWall(d) => {
                let progress = if let CoatWallPhase::Coating { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (SoulTaskPhaseVisual::CoatWall, progress, Some(d.tile), None)
            }
            AssignedTask::Refine(d) => (SoulTaskPhaseVisual::Refine, None, Some(d.mixer), None),
            AssignedTask::CollectSand(d) => {
                (SoulTaskPhaseVisual::CollectSand, None, Some(d.target), None)
            }
            AssignedTask::CollectBone(d) => {
                (SoulTaskPhaseVisual::CollectBone, None, Some(d.target), None)
            }
            AssignedTask::MovePlant(_) => (SoulTaskPhaseVisual::MovePlant, None, None, None),
            AssignedTask::HaulToMixer(d) => {
                (SoulTaskPhaseVisual::HaulToMixer, None, Some(d.mixer), None)
            }
            AssignedTask::HaulWithWheelbarrow(_) => {
                (SoulTaskPhaseVisual::HaulWithWheelbarrow, None, None, None)
            }
            AssignedTask::BucketTransport(d) => (
                SoulTaskPhaseVisual::BucketTransport,
                None,
                None,
                Some(d.bucket),
            ),
        };

        state.phase = phase;
        state.progress = progress;
        state.link_target = link_target;
        state.bucket_link = bucket_link;
    }
}

pub fn sync_blueprint_visual_system(mut q: BlueprintSyncQuery) {
    for (bp, mut state) in q.iter_mut() {
        state.progress = bp.progress;
        state.material_counts = bp
            .required_materials
            .iter()
            .map(|(rt, req)| {
                (
                    *rt,
                    bp.delivered_materials.get(rt).copied().unwrap_or(0),
                    *req,
                )
            })
            .collect();
        state.flexible_material = bp.flexible_material_requirement.as_ref().map(|f| {
            (
                f.accepted_types.clone(),
                f.delivered_total,
                f.required_total,
            )
        });
        state.is_wall_or_door = matches!(bp.kind, BuildingType::Wall | BuildingType::Door);
        state.is_plain_wall = matches!(bp.kind, BuildingType::Wall);
        state.occupied_grids = bp.occupied_grids.clone();
    }
}

pub fn sync_floor_tile_visual_system(mut q: FloorTileSyncQuery) {
    for (tile, mut mirror) in q.iter_mut() {
        mirror.bones_delivered = tile.bones_delivered;
        mirror.state = match tile.state {
            FloorTileState::WaitingBones => FloorTileStateMirror::WaitingBones,
            FloorTileState::ReinforcingReady => FloorTileStateMirror::ReinforcingReady,
            FloorTileState::Reinforcing { progress } => {
                FloorTileStateMirror::Reinforcing { progress }
            }
            FloorTileState::ReinforcedComplete => FloorTileStateMirror::ReinforcedComplete,
            FloorTileState::WaitingMud => FloorTileStateMirror::WaitingMud,
            FloorTileState::PouringReady => FloorTileStateMirror::PouringReady,
            FloorTileState::Pouring { progress } => FloorTileStateMirror::Pouring { progress },
            FloorTileState::Complete => FloorTileStateMirror::Complete,
        };
    }
}

pub fn sync_wall_tile_visual_system(mut q: WallTileSyncQuery) {
    for (tile, mut mirror) in q.iter_mut() {
        mirror.state = match tile.state {
            WallTileState::WaitingWood => WallTileStateMirror::WaitingWood,
            WallTileState::FramingReady => WallTileStateMirror::FramingReady,
            WallTileState::Framing { progress } => WallTileStateMirror::Framing { progress },
            WallTileState::FramedProvisional => WallTileStateMirror::FramedProvisional,
            WallTileState::WaitingMud => WallTileStateMirror::WaitingMud,
            WallTileState::CoatingReady => WallTileStateMirror::CoatingReady,
            WallTileState::Coating { progress } => WallTileStateMirror::Coating { progress },
            WallTileState::Complete => WallTileStateMirror::Complete,
        };
    }
}

pub fn sync_floor_site_visual_system(mut q: FloorSiteSyncQuery) {
    for (site, mut state) in q.iter_mut() {
        state.phase = match site.phase {
            FloorConstructionPhase::Reinforcing => FloorConstructionPhaseMirror::Reinforcing,
            FloorConstructionPhase::Pouring => FloorConstructionPhaseMirror::Pouring,
            FloorConstructionPhase::Curing => FloorConstructionPhaseMirror::Curing,
        };
        state.curing_remaining_secs = site.curing_remaining_secs;
        state.tiles_total = site.tiles_total;
    }
}

pub fn sync_wall_site_visual_system(mut q: WallSiteSyncQuery) {
    for (site, mut state) in q.iter_mut() {
        state.phase_is_framing = site.phase == WallConstructionPhase::Framing;
        state.tiles_total = site.tiles_total;
        state.tiles_framed = site.tiles_framed;
        state.tiles_coated = site.tiles_coated;
    }
}

/// Updates `BuildingVisualState` whenever `Building` changes.
pub fn sync_building_visual_system(
    mut q: Query<(&Building, &mut BuildingVisualState), Changed<Building>>,
) {
    for (building, mut state) in q.iter_mut() {
        state.kind = building_type_to_visual(building.kind);
        state.is_provisional = building.is_provisional;
    }
}

/// Scans all Soul `AssignedTask`s and updates each Mixer's `MudMixerVisualState`.
/// Full scan is necessary because the active state depends on other entities' state.
pub fn sync_mud_mixer_active_system(
    q_tasks: Query<&AssignedTask>,
    mut q_mixers: Query<(Entity, &mut MudMixerVisualState)>,
) {
    let refining_mixers: std::collections::HashSet<Entity> = q_tasks
        .iter()
        .filter_map(|task| match task {
            AssignedTask::Refine(data) if matches!(data.phase, RefinePhase::Refining { .. }) => {
                Some(data.mixer)
            }
            _ => None,
        })
        .collect();

    for (entity, mut state) in q_mixers.iter_mut() {
        let active = refining_mixers.contains(&entity);
        if state.is_active != active {
            state.is_active = active;
        }
    }
}
