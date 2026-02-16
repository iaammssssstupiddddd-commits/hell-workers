use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::systems::command::TaskArea;
use crate::systems::jobs::Building;
use bevy::prelude::*;

const TASK_AREA_BORDER_HIT_THICKNESS: f32 = 6.0;

pub(super) fn hovered_task_area_border_entity(
    world_pos: Vec2,
    selected_entity: Option<Entity>,
    q_task_areas: &Query<(Entity, &TaskArea), With<Familiar>>,
) -> Option<Entity> {
    if let Some(selected) = selected_entity
        && let Ok((_, area)) = q_task_areas.get(selected)
        && area.contains_border(world_pos, TASK_AREA_BORDER_HIT_THICKNESS)
    {
        return Some(selected);
    }

    q_task_areas
        .iter()
        .filter(|(_, area)| area.contains_border(world_pos, TASK_AREA_BORDER_HIT_THICKNESS))
        .min_by(|(_, area_a), (_, area_b)| {
            area_a
                .center()
                .distance_squared(world_pos)
                .partial_cmp(&area_b.center().distance_squared(world_pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, _)| entity)
}

pub(super) fn hovered_entity_at_world_pos(
    world_pos: Vec2,
    q_souls: &Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: &Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_targets: &Query<
        (Entity, &GlobalTransform, Option<&Building>),
        Or<(
            With<crate::systems::jobs::Tree>,
            With<crate::systems::jobs::Rock>,
            With<crate::systems::logistics::ResourceItem>,
            With<crate::systems::jobs::Building>,
        )>,
    >,
) -> Option<Entity> {
    // 1. 使い魔（優先）
    for (entity, transform) in q_familiars.iter() {
        let pos = transform.translation().truncate();
        if pos.distance(world_pos) < TILE_SIZE / 2.0 {
            return Some(entity);
        }
    }

    // 2. 魂
    for (entity, transform) in q_souls.iter() {
        let pos = transform.translation().truncate();
        if pos.distance(world_pos) < TILE_SIZE / 2.0 {
            return Some(entity);
        }
    }

    // 3. 資源・アイテム・建物
    for (entity, transform, building_opt) in q_targets.iter() {
        let pos = transform.translation().truncate();
        let radius = if let Some(building) = building_opt {
            match building.kind {
                crate::systems::jobs::BuildingType::Tank
                | crate::systems::jobs::BuildingType::MudMixer
                | crate::systems::jobs::BuildingType::RestArea => TILE_SIZE, // 2x2なので半径を大きく
                _ => TILE_SIZE / 2.0,
            }
        } else {
            TILE_SIZE / 2.0
        };

        if pos.distance(world_pos) < radius {
            return Some(entity);
        }
    }

    None
}

pub(super) fn selectable_worker_at_world_pos(
    world_pos: Vec2,
    q_souls: &Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: &Query<(Entity, &GlobalTransform), With<Familiar>>,
) -> Option<Entity> {
    for (entity, transform) in q_familiars.iter() {
        let pos = transform.translation().truncate();
        if pos.distance(world_pos) < TILE_SIZE / 2.0 {
            return Some(entity);
        }
    }

    for (entity, transform) in q_souls.iter() {
        let pos = transform.translation().truncate();
        if pos.distance(world_pos) < TILE_SIZE / 2.0 {
            return Some(entity);
        }
    }

    None
}
