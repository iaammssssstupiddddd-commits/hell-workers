use super::{super::*, validate_world_map_grid};

pub(in super::super) fn validate_energy_links(candidate: &World) -> Result<(), String> {
    let map = candidate
        .get_resource::<WorldMap>()
        .ok_or_else(|| "persisted WorldMap is missing".to_owned())?;
    let mut generators_by_grid: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut consumers_by_grid: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let mut soul_spa_tiles_by_site: HashMap<Entity, HashSet<(i32, i32)>> = HashMap::new();

    for source in candidate.iter_entities() {
        if source.contains::<SoulSpaSite>()
            && (source
                .get::<Building>()
                .is_none_or(|building| building.kind != BuildingType::SoulSpa)
                || !source.contains::<Transform>()
                || !source.contains::<PowerGenerator>())
        {
            return Err(format!(
                "SoulSpaSite {:?} is missing its SoulSpa Building, Transform, or PowerGenerator role",
                source.id()
            ));
        }
        if let Some(owner) = source.get::<YardPowerGrid>() {
            if !source.contains::<PowerGrid>() {
                return Err(format!(
                    "YardPowerGrid source {:?} is not a PowerGrid",
                    source.id()
                ));
            }
            let yard = candidate.get_entity(owner.0).map_err(|_| {
                format!(
                    "PowerGrid {:?} references missing Yard {:?}",
                    source.id(),
                    owner.0
                )
            })?;
            if !yard.contains::<Yard>() {
                return Err(format!(
                    "PowerGrid {:?} YardPowerGrid target {:?} is not a Yard",
                    source.id(),
                    owner.0
                ));
            }
        }
        if let Some(relation) = source.get::<GeneratesFor>() {
            if !source.contains::<PowerGenerator>() {
                return Err(format!(
                    "GeneratesFor source {:?} is not a PowerGenerator",
                    source.id()
                ));
            }
            validate_power_grid_target(candidate, relation.0, "GeneratesFor")?;
            generators_by_grid
                .entry(relation.0)
                .or_default()
                .insert(source.id());
        }
        if let Some(relation) = source.get::<ConsumesFrom>() {
            if !source.contains::<PowerConsumer>() {
                return Err(format!(
                    "ConsumesFrom source {:?} is not a PowerConsumer",
                    source.id()
                ));
            }
            validate_power_grid_target(candidate, relation.0, "ConsumesFrom")?;
            consumers_by_grid
                .entry(relation.0)
                .or_default()
                .insert(source.id());
        }
        if let Some(tile) = source.get::<SoulSpaTile>() {
            let site = candidate.get_entity(tile.parent_site).map_err(|_| {
                format!(
                    "SoulSpaTile {:?} references missing parent site {:?}",
                    source.id(),
                    tile.parent_site
                )
            })?;
            if !site.contains::<SoulSpaSite>() {
                return Err(format!(
                    "SoulSpaTile {:?} parent {:?} is not a SoulSpaSite",
                    source.id(),
                    tile.parent_site
                ));
            }
            validate_world_map_grid("SoulSpaTile.grid_pos", tile.grid_pos)?;
            if map.buildings.get(&tile.grid_pos) != Some(&tile.parent_site) {
                return Err(format!(
                    "SoulSpaTile {:?} grid {:?} is not owned by site {:?} in WorldMap.buildings",
                    source.id(),
                    tile.grid_pos,
                    tile.parent_site
                ));
            }
            if !source.contains::<Transform>() {
                return Err(format!("SoulSpaTile {:?} has no Transform", source.id()));
            }
            if !soul_spa_tiles_by_site
                .entry(tile.parent_site)
                .or_default()
                .insert(tile.grid_pos)
            {
                return Err(format!(
                    "SoulSpaSite {:?} has duplicate tile grid {:?}",
                    tile.parent_site, tile.grid_pos
                ));
            }
        }
    }

    for site in candidate.iter_entities() {
        if site.contains::<SoulSpaSite>() {
            let tile_count = soul_spa_tiles_by_site
                .get(&site.id())
                .map_or(0, HashSet::len);
            if tile_count != 4 {
                return Err(format!(
                    "SoulSpaSite {:?} owns {tile_count} tile(s), expected 4",
                    site.id()
                ));
            }
        }
    }

    for (&grid, &owner) in &map.buildings {
        if candidate.get::<SoulSpaSite>(owner).is_some()
            && !soul_spa_tiles_by_site
                .get(&owner)
                .is_some_and(|grids| grids.contains(&grid))
        {
            return Err(format!(
                "WorldMap.buildings grid {grid:?} points to SoulSpaSite {owner:?} outside its tile footprint"
            ));
        }
    }

    for grid in candidate.iter_entities() {
        validate_relationship_target(
            candidate,
            &grid,
            grid.get::<GridGenerators>()
                .map(|targets| targets.iter().copied()),
            generators_by_grid.get(&grid.id()),
            "GeneratesFor/GridGenerators",
            |source| source.contains::<PowerGenerator>(),
            |source| source.get::<GeneratesFor>().map(|relation| relation.0),
        )?;
        validate_relationship_target(
            candidate,
            &grid,
            grid.get::<GridConsumers>()
                .map(|targets| targets.iter().copied()),
            consumers_by_grid.get(&grid.id()),
            "ConsumesFrom/GridConsumers",
            |source| source.contains::<PowerConsumer>(),
            |source| source.get::<ConsumesFrom>().map(|relation| relation.0),
        )?;
    }
    Ok(())
}

fn validate_power_grid_target(
    candidate: &World,
    target: Entity,
    relation: &str,
) -> Result<(), String> {
    let grid = candidate
        .get_entity(target)
        .map_err(|_| format!("{relation} target {target:?} is missing"))?;
    if grid.contains::<PowerGrid>() {
        Ok(())
    } else {
        Err(format!("{relation} target {target:?} is not a PowerGrid"))
    }
}

fn validate_relationship_target<I, FRole, FTarget>(
    candidate: &World,
    target: &EntityRef<'_>,
    actual: Option<I>,
    expected: Option<&HashSet<Entity>>,
    relation: &str,
    source_has_role: FRole,
    source_target: FTarget,
) -> Result<(), String>
where
    I: Iterator<Item = Entity>,
    FRole: Fn(&EntityRef<'_>) -> bool,
    FTarget: Fn(&EntityRef<'_>) -> Option<Entity>,
{
    let Some(actual) = actual else {
        if expected.is_some_and(|sources| !sources.is_empty()) {
            return Err(format!(
                "{relation} target {:?} is missing its relationship target component",
                target.id()
            ));
        }
        return Ok(());
    };
    if !target.contains::<PowerGrid>() {
        return Err(format!(
            "{relation} target component is attached to non-PowerGrid {:?}",
            target.id()
        ));
    }
    let actual_vec: Vec<_> = actual.collect();
    let actual_set: HashSet<_> = actual_vec.iter().copied().collect();
    if actual_vec.len() != actual_set.len() {
        return Err(format!(
            "{relation} target {:?} contains duplicate sources",
            target.id()
        ));
    }
    if actual_set != expected.cloned().unwrap_or_default() {
        return Err(format!(
            "{relation} is not symmetric for PowerGrid {:?}",
            target.id()
        ));
    }
    for source in actual_set {
        let source_ref = candidate
            .get_entity(source)
            .map_err(|_| format!("{relation} references missing source {source:?}"))?;
        if !source_has_role(&source_ref) || source_target(&source_ref) != Some(target.id()) {
            return Err(format!(
                "{relation} source {source:?} has an invalid role or backlink"
            ));
        }
    }
    Ok(())
}
