use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::systems::command::TaskArea;
use crate::systems::jobs::Building;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;

const TASK_AREA_BORDER_HIT_THICKNESS: f32 = 6.0;
const TILE_HALF_SIZE_SQ: f32 = (TILE_SIZE / 2.0) * (TILE_SIZE / 2.0);

type SelectionTargetQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static GlobalTransform, Option<&'static Building>),
    Or<(
        With<crate::systems::jobs::Tree>,
        With<crate::systems::jobs::Rock>,
        With<crate::systems::logistics::ResourceItem>,
        With<crate::systems::jobs::Building>,
    )>,
>;

type ManagedStockpileQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static GlobalTransform), With<hw_logistics::StockpilePolicy>>;

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

fn entity_hit_radius(building_opt: Option<&Building>) -> f32 {
    if let Some(building) = building_opt {
        let radius = match building.kind {
            crate::systems::jobs::BuildingType::Tank
            | crate::systems::jobs::BuildingType::MudMixer
            | crate::systems::jobs::BuildingType::RestArea
            | crate::systems::jobs::BuildingType::SoulSpa => TILE_SIZE,
            crate::systems::jobs::BuildingType::Bridge => TILE_SIZE * 2.5,
            _ => TILE_SIZE / 2.0,
        };
        radius * radius
    } else {
        TILE_HALF_SIZE_SQ
    }
}

pub(super) fn hovered_entity_at_world_pos(
    world_pos: Vec2,
    q_souls: &Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: &Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_stockpile_cells: &ManagedStockpileQuery,
    q_targets: &SelectionTargetQuery,
) -> Option<Entity> {
    // 1. 使い魔（優先）
    for (entity, transform) in q_familiars.iter() {
        let pos = transform.translation().truncate();
        if pos.distance_squared(world_pos) < TILE_HALF_SIZE_SQ {
            return Some(entity);
        }
    }

    // 2. 魂
    for (entity, transform) in q_souls.iter() {
        let pos = transform.translation().truncate();
        if pos.distance_squared(world_pos) < TILE_HALF_SIZE_SQ {
            return Some(entity);
        }
    }

    // 3. Player-managed Stockpile cells. Stored ResourceItem entities share the same position,
    // so this dedicated tier must precede the generic target query.
    if let Some((entity, _)) = q_stockpile_cells
        .iter()
        .filter(|(_, transform)| {
            transform
                .translation()
                .truncate()
                .distance_squared(world_pos)
                < TILE_HALF_SIZE_SQ
        })
        .min_by(
            |(left_entity, left_transform), (right_entity, right_transform)| {
                left_transform
                    .translation()
                    .truncate()
                    .distance_squared(world_pos)
                    .total_cmp(
                        &right_transform
                            .translation()
                            .truncate()
                            .distance_squared(world_pos),
                    )
                    .then_with(|| {
                        (left_entity.index_u32(), left_entity.generation().to_bits()).cmp(&(
                            right_entity.index_u32(),
                            right_entity.generation().to_bits(),
                        ))
                    })
            },
        )
    {
        return Some(entity);
    }

    // 4. 資源・アイテム・建物
    for (entity, transform, building_opt) in q_targets.iter() {
        let pos = transform.translation().truncate();
        let radius_sq = entity_hit_radius(building_opt);

        if pos.distance_squared(world_pos) < radius_sq {
            return Some(entity);
        }
    }

    None
}
