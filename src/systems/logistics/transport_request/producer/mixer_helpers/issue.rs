use bevy::prelude::*;

use crate::events::{DesignationOp, DesignationRequest};
use crate::systems::jobs::{Designation, WorkType};
use crate::systems::world::zones::{AreaBounds, Yard};
use crate::world::map::{TerrainType, WorldMap};

use super::types::MixerCollectSandCandidate;

pub(crate) fn issue_collect_sand_if_needed(
    designation_writer: &mut MessageWriter<DesignationRequest>,
    candidate: &MixerCollectSandCandidate,
    q_sand_piles: &Query<
        (
            Entity,
            &Transform,
            Option<&Designation>,
            Option<&crate::relationships::TaskWorkers>,
        ),
        With<crate::systems::jobs::SandPile>,
    >,
    q_task_state: &Query<(
        Option<&Designation>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    world_map: &WorldMap,
) {
    if candidate.current_sand + candidate.sand_inflight >= 2 {
        return;
    }

    let mut issued_collect_sand = false;
    if let Some(area) = candidate.yard_area.as_ref() {
        for (sp_entity, sp_transform, sp_designation, sp_workers) in q_sand_piles.iter() {
            if !area.contains(sp_transform.translation.truncate()) {
                continue;
            }
            if sp_designation.is_some() || sp_workers.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            designation_writer.write(DesignationRequest {
                entity: sp_entity,
                operation: DesignationOp::Issue {
                    work_type: WorkType::CollectSand,
                    issued_by: candidate.issued_by,
                    task_slots: 1,
                    priority: Some(4),
                    target_blueprint: None,
                    target_mixer: None,
                    reserved_for_task: false,
                },
            });
            info!(
                "AUTO_HAUL_MIXER: Issued CollectSand from SandPile {:?} for Mixer {:?}",
                sp_entity, candidate.mixer_entity
            );
            issued_collect_sand = true;
            break;
        }
    }

    if issued_collect_sand {
        return;
    }

    let area_filters: [Option<&AreaBounds>; 2] = [Some(&candidate.owner_area), None];
    for area_filter in area_filters {
        for (sp_entity, sp_transform, sp_designation, sp_workers) in q_sand_piles.iter() {
            if area_filter.is_some_and(|area| !area.contains(sp_transform.translation.truncate())) {
                continue;
            }
            if sp_designation.is_some() || sp_workers.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            designation_writer.write(DesignationRequest {
                entity: sp_entity,
                operation: DesignationOp::Issue {
                    work_type: WorkType::CollectSand,
                    issued_by: candidate.issued_by,
                    task_slots: 1,
                    priority: Some(4),
                    target_blueprint: None,
                    target_mixer: None,
                    reserved_for_task: false,
                },
            });
            info!(
                "AUTO_HAUL_MIXER: Issued CollectSand from SandPile {:?} for Mixer {:?}",
                sp_entity, candidate.mixer_entity
            );
            issued_collect_sand = true;
            break;
        }
        if issued_collect_sand {
            break;
        }
    }

    if issued_collect_sand {
        return;
    }

    let sand_tile_searches = [
        (None, candidate.yard_area.as_ref()),
        (Some(&candidate.owner_area), None),
        (None, None),
    ];
    for (owner_area, yard_area) in sand_tile_searches {
        let Some(sand_tile_entity) = find_available_sand_tile(
            world_map,
            owner_area,
            yard_area,
            candidate.mixer_pos,
            q_task_state,
        ) else {
            continue;
        };

        designation_writer.write(DesignationRequest {
            entity: sand_tile_entity,
            operation: DesignationOp::Issue {
                work_type: WorkType::CollectSand,
                issued_by: candidate.issued_by,
                task_slots: 1,
                priority: Some(4),
                target_blueprint: None,
                target_mixer: None,
                reserved_for_task: false,
            },
        });
        info!(
            "AUTO_HAUL_MIXER: Issued CollectSand from beach tile {:?} for Mixer {:?}",
            sand_tile_entity, candidate.mixer_entity
        );
        break;
    }
}

pub(crate) fn find_available_sand_tile(
    world_map: &WorldMap,
    owner_area: Option<&AreaBounds>,
    yard_area: Option<&Yard>,
    mixer_pos: Vec2,
    q_task_state: &Query<(
        Option<&Designation>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
) -> Option<Entity> {
    let (min_x, max_x, min_y, max_y) = if let Some(yard) = yard_area {
        let (x0, y0) = WorldMap::world_to_grid(yard.min);
        let (x1, y1) = WorldMap::world_to_grid(yard.max);
        (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1))
    } else if let Some(area) = owner_area {
        let (x0, y0) = WorldMap::world_to_grid(area.min);
        let (x1, y1) = WorldMap::world_to_grid(area.max);
        (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1))
    } else {
        (
            0,
            hw_core::constants::MAP_WIDTH - 1,
            0,
            hw_core::constants::MAP_HEIGHT - 1,
        )
    };

    let mut best: Option<(Entity, f32)> = None;

    for gy in min_y..=max_y {
        for gx in min_x..=max_x {
            let Some(idx) = world_map.pos_to_idx(gx, gy) else {
                continue;
            };
            if world_map.terrain_at_idx(idx) != Some(TerrainType::Sand) {
                continue;
            }

            let Some(tile_entity) = world_map.tile_entity_at_idx(idx) else {
                continue;
            };
            let Ok((designation, workers)) = q_task_state.get(tile_entity) else {
                continue;
            };
            if designation.is_some() || workers.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            let tile_pos = WorldMap::grid_to_world(gx, gy);
            if yard_area.is_none() && owner_area.is_some_and(|area| !area.contains(tile_pos)) {
                continue;
            }

            let dist_sq = tile_pos.distance_squared(mixer_pos);
            match best {
                Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
                _ => best = Some((tile_entity, dist_sq)),
            }
        }
    }

    best.map(|(entity, _)| entity)
}
