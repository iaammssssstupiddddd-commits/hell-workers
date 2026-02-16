use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::jobs::WorkType;
use crate::systems::spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

pub(super) struct CandidateSnapshot {
    pub pos: Vec2,
    pub target_grid: (i32, i32),
    pub target_walkable: bool,
    pub skip_reachability_check: bool,
    pub work_type: WorkType,
    pub base_priority: i32,
    pub in_stockpile_none: bool,
}

/// タスク候補エンティティを収集する。
///
/// 計画: TransportRequestSpatialGrid を主に参照し、DesignationSpatialGrid と統合して
/// 「自分の TaskArea 内の request + 自分の ManagedRequests」を返す。
pub(super) fn collect_candidate_entities(
    task_area_opt: Option<&TaskArea>,
    managed_tasks: &ManagedTasks,
    designation_grid: &DesignationSpatialGrid,
    transport_request_grid: &TransportRequestSpatialGrid,
) -> Vec<Entity> {
    let mut seen = HashSet::new();

    if let Some(area) = task_area_opt {
        for &e in designation_grid.get_in_area(area.min, area.max).iter() {
            seen.insert(e);
        }
        for &e in transport_request_grid
            .get_in_area(area.min, area.max)
            .iter()
        {
            seen.insert(e);
        }
    }

    for &managed_entity in managed_tasks.iter() {
        seen.insert(managed_entity);
    }

    seen.into_iter().collect()
}

pub(super) fn candidate_snapshot(
    fam_entity: Entity,
    entity: Entity,
    task_area_opt: Option<&TaskArea>,
    managed_tasks: &ManagedTasks,
    world_map: &WorldMap,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<CandidateSnapshot> {
    let (
        _entity,
        transform,
        designation,
        issued_by,
        slots,
        workers,
        in_stockpile_opt,
        priority_opt,
    ) = queries.designation.designations.get(entity).ok()?;

    let is_managed_by_me = managed_tasks.contains(entity);
    let is_unassigned = issued_by.is_none();
    let is_issued_by_me = issued_by.map(|ib| ib.0) == Some(fam_entity);

    if !is_managed_by_me && !is_unassigned && !is_issued_by_me {
        return None;
    }

    let current_workers = workers.map(|w| w.len()).unwrap_or(0);
    let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
    if current_workers >= max_slots {
        return None;
    }

    let pos = transform.translation.truncate();
    let is_mixer_task = queries.storage.target_mixers.get(entity).is_ok();

    if let Some(area) = task_area_opt {
        if !area.contains(pos) {
            if !is_managed_by_me && !is_mixer_task {
                return None;
            }
        }
    } else if !is_managed_by_me && !is_mixer_task {
        return None;
    }

    let target_grid = WorldMap::world_to_grid(pos);
    let target_walkable = world_map.is_walkable(target_grid.0, target_grid.1);
    let is_transport_request = queries.transport_requests.get(entity).is_ok();
    let requires_transport_request = matches!(
        designation.work_type,
        WorkType::Haul
            | WorkType::HaulToMixer
            | WorkType::GatherWater
            | WorkType::HaulWaterToMixer
            | WorkType::WheelbarrowHaul
    );
    if requires_transport_request && !is_transport_request {
        return None;
    }

    let is_valid = match designation.work_type {
        WorkType::Chop
        | WorkType::Mine
        | WorkType::Haul
        | WorkType::HaulToMixer
        | WorkType::GatherWater
        | WorkType::CollectSand
        | WorkType::CollectBone
        | WorkType::Refine
        | WorkType::HaulWaterToMixer
        | WorkType::WheelbarrowHaul => true,
        WorkType::Build => {
            if let Ok((_, bp, _)) = queries.storage.blueprints.get(entity) {
                bp.materials_complete()
            } else {
                false
            }
        }
        WorkType::ReinforceFloorTile => {
            // Validate tile is in ReinforcingReady state
            if let Ok(tile) = queries
                .storage
                .floor_tiles
                .get(entity)
            {
                matches!(
                    tile.state,
                    crate::systems::jobs::floor_construction::FloorTileState::ReinforcingReady
                )
            } else {
                false
            }
        }
        WorkType::PourFloorTile => {
            // Validate tile is in PouringReady state
            if let Ok(tile) = queries
                .storage
                .floor_tiles
                .get(entity)
            {
                matches!(
                    tile.state,
                    crate::systems::jobs::floor_construction::FloorTileState::PouringReady
                )
            } else {
                false
            }
        }
    };

    if !is_valid {
        return None;
    }

    let base_priority = priority_opt.map(|p| p.0).unwrap_or(0) as i32;
    let in_stockpile_none = in_stockpile_opt.is_none();
    Some(CandidateSnapshot {
        pos,
        target_grid,
        target_walkable,
        // request タスクは source/destination を遅延解決するため、
        // request 自体の座標への到達判定を事前に強制しない。
        // Refine は 2x2 建物中心座標をターゲットにするため、
        // 事前判定での偽陰性を避けて実行側の到達判定に委ねる。
        skip_reachability_check: is_transport_request
            || matches!(designation.work_type, WorkType::Refine),
        work_type: designation.work_type,
        base_priority,
        in_stockpile_none,
    })
}
