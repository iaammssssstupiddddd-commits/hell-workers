use super::super::*;

pub(super) fn validate_durable_owner_links(candidate: &World) -> Result<(), String> {
    for source in candidate.iter_entities() {
        let belongs_to = source.get::<BelongsTo>();
        let pending = source.get::<PendingBelongsToBlueprint>();
        if belongs_to.is_some() && pending.is_some() {
            return Err(format!(
                "entity {:?} has both BelongsTo and PendingBelongsToBlueprint",
                source.id()
            ));
        }

        if let Some(owner) = belongs_to {
            let target = candidate.get_entity(owner.0).map_err(|_| {
                format!(
                    "BelongsTo source {:?} references missing owner {:?}",
                    source.id(),
                    owner.0
                )
            })?;
            if source.contains::<Wheelbarrow>() {
                if !target.contains::<WheelbarrowParking>() {
                    return Err(format!(
                        "Wheelbarrow {:?} BelongsTo target {:?} is not WheelbarrowParking",
                        source.id(),
                        owner.0
                    ));
                }
            } else if source.contains::<Stockpile>() {
                if !target.contains::<Yard>() && !is_tank_building(&target) {
                    return Err(format!(
                        "Stockpile {:?} BelongsTo target {:?} is neither Yard nor Tank",
                        source.id(),
                        owner.0
                    ));
                }
            } else if let Some(item) = source.get::<ResourceItem>() {
                let valid_owner = if matches!(
                    item.0,
                    ResourceType::BucketEmpty | ResourceType::BucketWater
                ) {
                    is_tank_building(&target)
                } else {
                    target.contains::<Familiar>() || target.contains::<Yard>()
                };
                if !valid_owner {
                    return Err(format!(
                        "ResourceItem {:?} has an incompatible BelongsTo owner {:?}",
                        source.id(),
                        owner.0
                    ));
                }
            } else {
                return Err(format!(
                    "BelongsTo source {:?} has no supported durable owner role",
                    source.id()
                ));
            }
        }

        if let Some(pending) = pending {
            let blueprint = candidate.get_entity(pending.0).map_err(|_| {
                format!(
                    "PendingBelongsToBlueprint source {:?} references missing Blueprint {:?}",
                    source.id(),
                    pending.0
                )
            })?;
            if !source.contains::<Stockpile>() || !is_tank_blueprint(&blueprint) {
                return Err(format!(
                    "PendingBelongsToBlueprint source {:?} is not a Tank companion Stockpile",
                    source.id()
                ));
            }
        }
    }
    Ok(())
}

fn is_tank_building(entity: &EntityRef<'_>) -> bool {
    entity
        .get::<Building>()
        .is_some_and(|building| building.kind == BuildingType::Tank)
}

fn is_tank_blueprint(entity: &EntityRef<'_>) -> bool {
    entity
        .get::<Blueprint>()
        .is_some_and(|blueprint| blueprint.kind == BuildingType::Tank)
}

pub(super) fn is_bucket_storage_role(candidate: &World, storage: &EntityRef<'_>) -> bool {
    storage.contains::<BucketStorage>()
        || storage
            .get::<BelongsTo>()
            .and_then(|owner| candidate.get_entity(owner.0).ok())
            .is_some_and(|owner| is_tank_building(&owner))
        || storage
            .get::<PendingBelongsToBlueprint>()
            .and_then(|owner| candidate.get_entity(owner.0).ok())
            .is_some_and(|owner| is_tank_blueprint(&owner))
}

pub(super) fn validate_managed_by(candidate: &World) -> Result<(), String> {
    for task in candidate.iter_entities() {
        let Some(managed_by) = task.get::<ManagedBy>() else {
            continue;
        };
        let manager = candidate
            .get_entity(managed_by.0)
            .map_err(|_| format!("ManagedBy references missing Familiar {:?}", managed_by.0))?;
        if !manager.contains::<Familiar>() && !manager.contains::<Yard>() {
            return Err(format!(
                "ManagedBy target {:?} is neither a Familiar nor a Yard",
                managed_by.0
            ));
        }
        if !manager
            .get::<ManagedTasks>()
            .is_some_and(|tasks| tasks.contains(task.id()))
        {
            return Err(format!(
                "ManagedBy/ManagedTasks are not symmetric for task {:?}",
                task.id()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_managed_tasks(candidate: &World) -> Result<(), String> {
    for manager in candidate.iter_entities() {
        let Some(tasks) = manager.get::<ManagedTasks>() else {
            continue;
        };
        if !manager.contains::<Familiar>() && !manager.contains::<Yard>() {
            return Err(format!(
                "ManagedTasks target {:?} is neither a Familiar nor a Yard",
                manager.id()
            ));
        }
        let unique: HashSet<_> = tasks.iter().collect();
        if unique.len() != tasks.len() {
            return Err(format!(
                "ManagedTasks for owner {:?} contains duplicate sources",
                manager.id()
            ));
        }
        for task in tasks.iter() {
            if candidate
                .get::<ManagedBy>(*task)
                .is_none_or(|managed_by| managed_by.0 != manager.id())
            {
                return Err(format!(
                    "ManagedTasks contains task {task:?} without the matching ManagedBy source"
                ));
            }
        }
    }
    Ok(())
}
