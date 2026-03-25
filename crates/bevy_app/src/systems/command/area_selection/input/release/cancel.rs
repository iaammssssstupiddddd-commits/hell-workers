use super::super::super::apply::apply_designation_in_area;
use super::super::super::cancel::cancel_single_designation;
use super::super::super::queries::{
    DesignationTargetQuery, FloorTileBlueprintQuery, WallTileBlueprintQuery,
};
use crate::app_contexts::TaskContext;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::floor_construction::FloorConstructionCancelRequested;
use crate::systems::jobs::wall_construction::WallConstructionCancelRequested;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use std::collections::HashSet;

pub(super) fn handle_release_cancel_designation(
    task_context: &mut TaskContext,
    selected_entity: Option<Entity>,
    world_pos: Vec2,
    start_pos: Vec2,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        FloorTileBlueprintQuery<'_, '_>,
        WallTileBlueprintQuery<'_, '_>,
    )>,
    commands: &mut Commands,
) {
    let end_pos = WorldMap::snap_to_grid_edge(world_pos);
    let drag_distance = start_pos.distance(end_pos);

    if drag_distance < TILE_SIZE * 0.5 {
        cancel_point(start_pos, q_target_sets, commands);
    } else {
        cancel_area(start_pos, end_pos, selected_entity, q_target_sets, commands);
    }

    task_context.0 = TaskMode::CancelDesignation(None);
}

fn cancel_point(
    start_pos: Vec2,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        FloorTileBlueprintQuery<'_, '_>,
        WallTileBlueprintQuery<'_, '_>,
    )>,
    commands: &mut Commands,
) {
    cancel_point_nearest_designation(start_pos, q_target_sets, commands);
    cancel_point_construction_sites(start_pos, q_target_sets, commands);
}

fn cancel_point_nearest_designation(
    start_pos: Vec2,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        FloorTileBlueprintQuery<'_, '_>,
        WallTileBlueprintQuery<'_, '_>,
    )>,
    commands: &mut Commands,
) {
    let mut closest: Option<(Entity, f32)> = None;
    {
        let q_targets = q_target_sets.p0();
        for (entity, transform, _, _, _, designation, _, _, _, _, _, _, _, _, _) in q_targets.iter()
        {
            if designation.is_none() {
                continue;
            }
            let dist = transform.translation.truncate().distance(start_pos);
            if dist < TILE_SIZE
                && closest.is_none_or(|(_, d)| dist < d) {
                    closest = Some((entity, dist));
                }
        }
    }

    if let Some((target_entity, _)) = closest {
        let q_targets = q_target_sets.p0();
        if let Ok((
            _,
            _,
            _,
            _,
            _,
            _,
            task_workers,
            blueprint,
            _,
            transport_request,
            fixed_source,
            _,
            _,
            _,
            _,
        )) = q_targets.get(target_entity)
        {
            cancel_single_designation(
                commands,
                target_entity,
                task_workers,
                blueprint.is_some(),
                transport_request.is_some(),
                fixed_source.map(|s| s.0),
            );
        }
    }
}

fn cancel_point_construction_sites(
    start_pos: Vec2,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        FloorTileBlueprintQuery<'_, '_>,
        WallTileBlueprintQuery<'_, '_>,
    )>,
    commands: &mut Commands,
) {
    let mut closest_floor: Option<(Entity, f32)> = None;
    {
        let q_floor = q_target_sets.p1();
        for (_, transform, tile) in q_floor.iter() {
            let dist = transform.translation.truncate().distance(start_pos);
            if dist <= TILE_SIZE {
                match closest_floor {
                    Some((_, best)) if best <= dist => {}
                    _ => closest_floor = Some((tile.parent_site, dist)),
                }
            }
        }
    }
    if let Some((site_entity, _)) = closest_floor {
        commands
            .entity(site_entity)
            .insert(FloorConstructionCancelRequested);
    }

    let mut closest_wall: Option<(Entity, f32)> = None;
    {
        let q_wall = q_target_sets.p2();
        for (_, transform, tile) in q_wall.iter() {
            let dist = transform.translation.truncate().distance(start_pos);
            if dist <= TILE_SIZE {
                match closest_wall {
                    Some((_, best)) if best <= dist => {}
                    _ => closest_wall = Some((tile.parent_site, dist)),
                }
            }
        }
    }
    if let Some((site_entity, _)) = closest_wall {
        commands
            .entity(site_entity)
            .insert(WallConstructionCancelRequested);
    }
}

fn cancel_area(
    start_pos: Vec2,
    end_pos: Vec2,
    selected_entity: Option<Entity>,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        FloorTileBlueprintQuery<'_, '_>,
        WallTileBlueprintQuery<'_, '_>,
    )>,
    commands: &mut Commands,
) {
    let area = TaskArea::from_points(start_pos, end_pos);

    {
        let q_targets = q_target_sets.p0();
        apply_designation_in_area(
            commands,
            TaskMode::CancelDesignation(Some(start_pos)),
            &area,
            selected_entity,
            &q_targets,
        );
    }

    cancel_area_construction_sites(&area, q_target_sets, commands);
}

fn cancel_area_construction_sites(
    area: &TaskArea,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        FloorTileBlueprintQuery<'_, '_>,
        WallTileBlueprintQuery<'_, '_>,
    )>,
    commands: &mut Commands,
) {
    let mut floor_sites = HashSet::new();
    {
        let q_floor = q_target_sets.p1();
        for (_, transform, tile) in q_floor.iter() {
            if area.contains(transform.translation.truncate()) {
                floor_sites.insert(tile.parent_site);
            }
        }
    }
    for site in floor_sites {
        commands
            .entity(site)
            .insert(FloorConstructionCancelRequested);
    }

    let mut wall_sites = HashSet::new();
    {
        let q_wall = q_target_sets.p2();
        for (_, transform, tile) in q_wall.iter() {
            if area.contains(transform.translation.truncate()) {
                wall_sites.insert(tile.parent_site);
            }
        }
    }
    for site in wall_sites {
        commands
            .entity(site)
            .insert(WallConstructionCancelRequested);
    }
}
