use super::*;
use hw_logistics::{BelongsTo, PendingBelongsToBlueprint, Stockpile};

fn validate_station(
    world: &World,
    target: Entity,
    district: ContractDistrict,
) -> Result<(), String> {
    let position = world
        .get::<Transform>(target)
        .ok_or("waterworks target has no position")?;
    let footprint = tank_footprint(district, position);
    if footprint.is_empty() {
        return Err("waterworks target outside district".into());
    }
    let pending = match (
        world.get::<Blueprint>(target),
        world.get::<Building>(target),
    ) {
        (Some(bp), None)
            if bp.kind == BuildingType::Tank
                && bp.occupied_grids.len() == footprint.len()
                && footprint
                    .iter()
                    .all(|cell| bp.occupied_grids.contains(cell)) =>
        {
            true
        }
        (None, Some(building))
            if world
                .get::<Stockpile>(target)
                .is_some_and(|pile| stations::is_completed_water_tank(building, pile)) =>
        {
            false
        }
        _ => return Err("waterworks requires one tank blueprint or completed tank".into()),
    };
    let mut cells = Vec::with_capacity(2);
    for entity in world.iter_entities().filter(|e| e.contains::<Stockpile>()) {
        let owned = if pending {
            entity
                .get::<PendingBelongsToBlueprint>()
                .is_some_and(|o| o.0 == target)
        } else {
            entity.get::<BelongsTo>().is_some_and(|o| o.0 == target)
        };
        if owned {
            let transform = entity
                .get::<Transform>()
                .ok_or("waterworks storage has no position")?;
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            if !transform.translation.is_finite() || !district.contains(grid) || cells.len() == 2 {
                return Err("invalid waterworks storage cells".into());
            }
            cells.push(grid);
        }
    }
    if stations::station_inside(district, &footprint, cells.into_iter()) {
        Ok(())
    } else {
        Err("waterworks requires one two-cell bucket storage".into())
    }
}

pub(crate) fn validate_candidate(world: &World) -> Result<(), String> {
    let mut contract = None;
    let mut target = None;
    for entity in world.iter_entities() {
        if let Some(record) = entity.get::<WaterworksContract>()
            && (entity.archetype().components().len() != 1 || contract.replace(record).is_some())
        {
            return Err("waterworks requires one separate contract record".into());
        }
        if let Some(tag) = entity.get::<WaterworksTarget>()
            && target.replace((entity.id(), tag.0)).is_some()
        {
            return Err("invalid waterworks target".into());
        }
    }
    let Some(record) = contract else {
        return if target.is_none() {
            Ok(())
        } else {
            Err("orphan waterworks target".into())
        };
    };
    let now = world
        .get_resource::<GameTime>()
        .ok_or("waterworks requires a saved clock")?
        .seconds;
    let valid_time = now.is_finite()
        && record.accepted <= now
        && record.accepted.is_finite()
        && record.accepted >= 0.0
        && record.deadline.is_finite()
        && record.deadline >= record.accepted;
    let d = record.district;
    if !valid_time
        || d.min.min_element() < 0
        || !d.min.cmple(d.max).all()
        || d.max.x >= hw_core::constants::MAP_WIDTH
        || d.max.y >= hw_core::constants::MAP_HEIGHT
        || record.full_reward < record.late_reward
    {
        return Err("invalid waterworks terms".into());
    }
    if let Some((entity, _)) = target {
        validate_station(world, entity, d)?;
    }
    match record.outcome {
        ContractOutcome::Active
            if target.is_none_or(|(_, id)| id > 0 && id == record.designation) =>
        {
            Ok(())
        }
        ContractOutcome::Inspected { at, reward } | ContractOutcome::Paid { at, reward }
            if target.is_none()
                && at.is_finite()
                && at >= record.accepted
                && at <= now
                && reward == record.reward(at) =>
        {
            Ok(())
        }
        _ => Err("inconsistent waterworks progress or reward".into()),
    }
}
