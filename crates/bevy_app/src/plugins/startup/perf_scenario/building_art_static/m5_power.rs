//! Actual seeded Spa -> worker -> grid -> Lamp context; no derived-state writes.
use super::*;
use hw_core::relationships::WorkingOn;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, YardPowerGrid,
};
use hw_jobs::GeneratePowerPhase;

pub(super) fn snapshot(
    world: &mut World,
    expected_workers: &[Entity],
) -> Result<Option<Value>, String> {
    let state = world.resource::<BuildingArtStaticState>();
    let sources = state
        .specs
        .iter()
        .zip(&state.owners)
        .filter(|(spec, _)| spec.kind == BuildingType::SoulSpa)
        .map(|(spec, owner)| (spec.clone(), *owner))
        .collect::<Vec<_>>();
    let lamps = state
        .specs
        .iter()
        .zip(&state.owners)
        .filter(|(spec, _)| spec.kind == BuildingType::OutdoorLamp)
        .map(|(spec, owner)| (spec.clone(), *owner))
        .collect::<Vec<_>>();
    if sources.len() != 2 || lamps.len() != 4 || expected_workers.len() != 6 {
        return Err("Lamp context source inventory differs".into());
    }
    let source_tiles = world
        .query::<(Entity, &SoulSpaTile)>()
        .iter(world)
        .map(|(entity, tile)| (entity, *tile))
        .collect::<Vec<_>>();
    if source_tiles.len() != 8 {
        return Err("Lamp context source tile inventory differs".into());
    }
    let mut records = Vec::new();
    let mut workers_seen = HashSet::new();
    let mut grids_seen = HashSet::new();
    for (spec, lamp) in &lamps {
        let Some(ConsumesFrom(grid)) = world.get::<ConsumesFrom>(*lamp) else {
            return Ok(None);
        };
        let grid = *grid;
        let Some(power) = world.get::<PowerGrid>(grid) else {
            return Ok(None);
        };
        let Some(yard_owner) = world.get::<YardPowerGrid>(grid) else {
            return Ok(None);
        };
        let yard = world
            .get::<hw_world::Yard>(yard_owner.0)
            .ok_or("Lamp context grid Yard missing")?;
        let consumers = world
            .get::<GridConsumers>(grid)
            .map(|value| value.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let generators = world
            .get::<GridGenerators>(grid)
            .map(|value| value.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let consumer = world
            .get::<PowerConsumer>(*lamp)
            .ok_or("Lamp context demand missing")?;
        let Some(visual) = world.get::<PoweredVisualState>(*lamp) else {
            return Ok(None);
        };
        if !grids_seen.insert(grid)
            || consumers != [*lamp]
            || !yard.contains(spec.center)
            || consumer.demand != hw_energy::constants::OUTDOOR_LAMP_DEMAND
        {
            return Err("Lamp context consumer/grid ownership differs".into());
        }
        let powered = spec.ordinal >= 2;
        let mut source_record = Value::Null;
        let output = if powered {
            let matching = sources
                .iter()
                .filter(|(source, _)| source.ordinal == spec.ordinal)
                .collect::<Vec<_>>();
            let [(source, owner)] = matching.as_slice() else {
                return Err("Lamp source ordinal differs".into());
            };
            if generators != [*owner]
                || !yard.contains(source.center)
                || world.get::<GeneratesFor>(*owner).map(|value| value.0) != Some(grid)
            {
                return Err("Lamp context generator/grid ownership differs".into());
            }
            let site = world
                .get::<SoulSpaSite>(*owner)
                .ok_or("Lamp context Spa missing")?;
            if world.get::<Building>(*owner).is_none_or(|building| {
                building.kind != BuildingType::SoulSpa || building.is_provisional
            }) || world
                .get::<Transform>(*owner)
                .is_none_or(|pose| pose.translation.truncate() != source.center)
            {
                return Err("Lamp context source owner differs".into());
            }
            let generator = world
                .get::<PowerGenerator>(*owner)
                .ok_or("Lamp context generator missing")?;
            let expected = source.spa_mask().count_ones() as usize;
            if site.phase != SoulSpaPhase::Operational
                || generator.output_per_soul != hw_energy::constants::OUTPUT_PER_SOUL
            {
                return Err("Lamp context generator contract differs".into());
            }
            let mut tiles = Vec::new();
            for (index, grid_pos) in source.tiles.iter().enumerate() {
                let found = source_tiles
                    .iter()
                    .filter(|(_, tile)| tile.parent_site == *owner && tile.grid_pos == *grid_pos)
                    .map(|(entity, _)| *entity)
                    .collect::<Vec<_>>();
                let [tile] = found.as_slice() else {
                    return Ok(None);
                };
                if world.get::<ChildOf>(*tile).map(ChildOf::parent) != Some(*owner)
                    || world
                        .resource::<hw_world::WorldMap>()
                        .building_entity(*grid_pos)
                        != Some(*owner)
                {
                    return Err("Lamp source tile hierarchy/footprint differs".into());
                }
                let workers = world
                    .get::<TaskWorkers>(*tile)
                    .map(|value| value.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default();
                if workers.len() != usize::from(source.spa_mask() & (1 << index) != 0) {
                    return Err("Lamp context exact source workers differ".into());
                }
                let mut worker_observations = Vec::new();
                for worker in &workers {
                    let task_matches = matches!(world.get::<hw_jobs::AssignedTask>(*worker),
                        Some(hw_jobs::AssignedTask::GeneratePower(data)) if data.tile == *tile && data.phase == GeneratePowerPhase::Generating
                            && data.tile_pos == hw_world::WorldMap::grid_to_world(grid_pos.0, grid_pos.1));
                    if !expected_workers.contains(worker)
                        || !workers_seen.insert(*worker)
                        || world.get::<WorkingOn>(*worker).map(|value| value.0) != Some(*tile)
                        || !task_matches
                    {
                        return Err("Lamp context source worker relationship differs".into());
                    }
                    let soul = world
                        .get::<hw_core::soul::DamnedSoul>(*worker)
                        .ok_or("Lamp source Soul missing")?;
                    let transform = world
                        .get::<Transform>(*worker)
                        .ok_or("Lamp source Soul pose missing")?;
                    let position = hw_world::WorldMap::grid_to_world(grid_pos.0, grid_pos.1);
                    if transform.translation.truncate() != position || soul.dream != 100.0 {
                        return Err("Lamp source Soul seeded pose/vitals changed".into());
                    }
                    worker_observations.push(json!({"entity": worker.to_bits(), "working_on": tile.to_bits(),
                        "task_tile": tile.to_bits(), "task_phase": "Generating", "position": position.to_array(), "dream": soul.dream}));
                }
                tiles.push(json!({"entity": tile.to_bits(), "parent": owner.to_bits(), "grid": [grid_pos.0, grid_pos.1],
                    "map_owner": owner.to_bits(), "workers": workers.iter().map(|worker| worker.to_bits()).collect::<Vec<_>>(), "worker_observations": worker_observations}));
            }
            let output = expected as f32 * generator.output_per_soul;
            if generator.current_output != output {
                return Ok(None);
            }
            source_record = json!({"entity": owner.to_bits(), "ordinal": source.ordinal, "mask": source.spa_mask(),
                "tiles": tiles, "output": generator.current_output, "output_per_soul": generator.output_per_soul,
                "grid": grid.to_bits(), "phase": "Operational", "worker_count": expected});
            output
        } else {
            if !generators.is_empty() {
                return Err("Lamp off control has a foreign generator".into());
            }
            0.0
        };
        if power.generation != output
            || power.consumption != consumer.demand
            || power.powered != powered
            || visual.is_powered != powered
        {
            return Ok(None);
        }
        records.push(json!({"lamp": lamp.to_bits(), "ordinal": spec.ordinal, "grid": grid.to_bits(),
            "yard": yard_owner.0.to_bits(), "yard_min": yard.min.to_array(), "yard_max": yard.max.to_array(),
            "generators": generators.iter().map(|entity| entity.to_bits()).collect::<Vec<_>>(),
            "consumers": consumers.iter().map(|entity| entity.to_bits()).collect::<Vec<_>>(),
            "generation": power.generation, "consumption": power.consumption, "demand": consumer.demand,
            "powered": powered, "source": source_record}));
    }
    if workers_seen.len() != 6 {
        return Err("Lamp context source worker inventory differs".into());
    }
    Ok(Some(
        json!({"records": records, "source_count": 2, "worker_count": 6}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::soul::DamnedSoul;
    use hw_jobs::{AssignedTask, GeneratePowerData};

    fn fixture() -> (World, Vec<Entity>) {
        let mut world = World::new();
        world.init_resource::<hw_world::WorldMap>();
        let specs = layout::layout(super::super::super::PerfScenarioSize::Small)
            .into_iter()
            .filter(|spec| {
                spec.kind == BuildingType::OutdoorLamp
                    || (spec.kind == BuildingType::SoulSpa && spec.ordinal >= 2)
            })
            .collect::<Vec<_>>();
        let mut owners = Vec::new();
        let mut grids = Vec::new();
        for ordinal in 0..4 {
            let x = 8 + ordinal * 5;
            let yard = world
                .spawn(hw_world::Yard {
                    min: hw_world::WorldMap::grid_to_world(x - 1, if ordinal < 2 { 47 } else { 5 }),
                    max: hw_world::WorldMap::grid_to_world(x + 2, 50),
                })
                .id();
            grids.push(
                world
                    .spawn((
                        PowerGrid {
                            generation: if ordinal == 2 {
                                2.
                            } else if ordinal == 3 {
                                4.
                            } else {
                                0.
                            },
                            consumption: hw_energy::constants::OUTDOOR_LAMP_DEMAND,
                            powered: ordinal >= 2,
                        },
                        YardPowerGrid(yard),
                    ))
                    .id(),
            );
        }
        let mut workers = Vec::new();
        for spec in &specs {
            let grid = grids[spec.ordinal];
            let owner = world
                .spawn((
                    Building {
                        kind: spec.kind,
                        is_provisional: false,
                    },
                    Transform::from_translation(spec.center.extend(0.)),
                ))
                .id();
            owners.push(owner);
            if spec.kind == BuildingType::OutdoorLamp {
                world.entity_mut(owner).insert((
                    ConsumesFrom(grid),
                    PowerConsumer {
                        demand: hw_energy::constants::OUTDOOR_LAMP_DEMAND,
                    },
                    PoweredVisualState {
                        is_powered: spec.ordinal >= 2,
                    },
                ));
            } else {
                world.entity_mut(owner).insert((
                    SoulSpaSite {
                        phase: SoulSpaPhase::Operational,
                        ..default()
                    },
                    PowerGenerator {
                        current_output: spec.spa_mask().count_ones() as f32,
                        ..default()
                    },
                    GeneratesFor(grid),
                ));
                for (index, &pos) in spec.tiles.iter().enumerate() {
                    let tile = world
                        .spawn((
                            SoulSpaTile {
                                parent_site: owner,
                                grid_pos: pos,
                            },
                            ChildOf(owner),
                        ))
                        .id();
                    world
                        .resource_mut::<hw_world::WorldMap>()
                        .set_building(pos, owner);
                    if spec.spa_mask() & (1 << index) != 0 {
                        let point = hw_world::WorldMap::grid_to_world(pos.0, pos.1);
                        workers.push(
                            world
                                .spawn((
                                    DamnedSoul {
                                        dream: 100.,
                                        ..default()
                                    },
                                    Transform::from_translation(point.extend(0.)),
                                    WorkingOn(tile),
                                    AssignedTask::GeneratePower(GeneratePowerData {
                                        tile,
                                        tile_pos: point,
                                        phase: GeneratePowerPhase::Generating,
                                    }),
                                ))
                                .id(),
                        );
                    }
                }
            }
        }
        world.insert_resource(BuildingArtStaticState {
            specs,
            owners,
            ..default()
        });
        (world, workers)
    }

    #[test]
    fn exact_seeded_producer_graph_is_read_only_and_rejects_mutation() {
        let (mut world, workers) = fixture();
        let first = snapshot(&mut world, &workers).unwrap().unwrap();
        assert_eq!(snapshot(&mut world, &workers).unwrap(), Some(first));
        let source = world.resource::<BuildingArtStaticState>().owners[0];
        world
            .get_mut::<PowerGenerator>(source)
            .unwrap()
            .current_output = 0.;
        assert!(snapshot(&mut world, &workers).unwrap().is_none());
        assert_eq!(
            world.get::<PowerGenerator>(source).unwrap().current_output,
            0.
        );
    }

    #[test]
    fn foreign_source_grid_and_worker_are_rejected() {
        let (mut world, workers) = fixture();
        let source = world.resource::<BuildingArtStaticState>().owners[0];
        let foreign = world.spawn_empty().id();
        world.entity_mut(source).insert(GeneratesFor(foreign));
        assert!(snapshot(&mut world, &workers).is_err());
        let (mut world, workers) = fixture();
        world.entity_mut(workers[0]).remove::<WorkingOn>();
        assert!(snapshot(&mut world, &workers).is_err());
    }
}
