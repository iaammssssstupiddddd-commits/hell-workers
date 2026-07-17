use super::*;

#[cfg(feature = "profiling")]
#[derive(Resource, Default)]
pub(crate) struct PerfScenarioApplied(pub(crate) bool);

/// Stable fixture identity used by fixed-step audit records. The marker avoids
/// treating allocator-dependent Entity IDs as part of the reproducibility
/// contract while still proving that the selected workload was installed.
#[cfg(feature = "profiling")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PerfFixtureMarker {
    pub(super) kind: PerfFixtureKind,
    pub(super) ordinal: u32,
}

#[cfg(feature = "profiling")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PerfFixtureKind {
    Door,
    ConstructionSite,
    ConstructionTile,
    UiBlueprint,
}

#[cfg(feature = "profiling")]
impl PerfFixtureKind {
    pub(super) const fn audit_tag(self) -> u8 {
        match self {
            Self::Door => 0,
            Self::ConstructionSite => 1,
            Self::ConstructionTile => 2,
            Self::UiBlueprint => 3,
        }
    }
}

/// Driver state intentionally holds no Entity IDs so it is world-epoch safe.
#[cfg(feature = "profiling")]
#[derive(Resource, Default)]
pub(crate) struct PerfScenarioDriverState {
    pub(super) last_path_door_toggle_slot: Option<u64>,
}

#[cfg(feature = "profiling")]
type PerfSetupFamiliarQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut ActiveCommand,
        &'static mut FamiliarOperation,
    ),
>;
#[cfg(feature = "profiling")]
type PerfSetupSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static mut AssignedTask,
    ),
>;
#[cfg(feature = "profiling")]
type PerfTreeQuery<'w, 's> = Query<'w, 's, Entity, With<Tree>>;
#[cfg(feature = "profiling")]
type PerfRockQuery<'w, 's> = Query<'w, 's, Entity, With<Rock>>;

#[cfg(feature = "profiling")]
#[derive(SystemParam)]
pub struct PerfWorkloadSetupParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    commands: Commands<'w, 's>,
    applied: ResMut<'w, PerfScenarioApplied>,
    q_familiars: PerfSetupFamiliarQuery<'w, 's>,
    q_souls: PerfSetupSoulQuery<'w, 's>,
    q_trees: PerfTreeQuery<'w, 's>,
    q_rocks: PerfRockQuery<'w, 's>,
    world_map: WorldMapWrite<'w>,
}

#[cfg(feature = "profiling")]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PerfScenarioSet {
    FixtureSpawn,
    FixtureApply,
    Setup,
    Apply,
    InitialCheckpoint,
    Driver,
    #[cfg(feature = "profiling")]
    Capture,
}

#[cfg(feature = "profiling")]
pub fn setup_perf_scenario_if_enabled(params: PerfWorkloadSetupParams) {
    setup_perf_workload_if_needed(params);
}

#[cfg(feature = "profiling")]
fn setup_perf_workload_if_needed(params: PerfWorkloadSetupParams) {
    let PerfWorkloadSetupParams {
        config,
        mut commands,
        mut applied,
        mut q_familiars,
        mut q_souls,
        q_trees,
        q_rocks,
        mut world_map,
    } = params;

    if applied.0 || !config.enabled() || q_familiars.is_empty() {
        return;
    }

    applied.0 = configure_perf_workload(
        &config,
        &mut commands,
        &mut q_familiars,
        &mut q_souls,
        &q_trees,
        &q_rocks,
        &mut world_map,
    );
}

#[cfg(feature = "profiling")]
pub fn setup_perf_scenario_runtime_if_enabled(params: PerfWorkloadSetupParams) {
    setup_perf_workload_if_needed(params);
}

#[cfg(feature = "profiling")]
fn configure_perf_workload(
    config: &PerfScenarioConfig,
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_souls: &mut Query<(
        Entity,
        &mut Transform,
        &mut Destination,
        &mut Path,
        &mut AssignedTask,
    )>,
    q_trees: &Query<Entity, With<Tree>>,
    q_rocks: &Query<Entity, With<Rock>>,
    world_map: &mut WorldMapWrite,
) -> bool {
    match config.workload {
        PerfWorkload::Gather => {
            configure_gather_baseline(commands, q_familiars, q_trees, q_rocks);
            true
        }
        PerfWorkload::PathDoor => {
            configure_path_door_fixture(commands, q_familiars, q_souls, world_map)
        }
        PerfWorkload::Construction => {
            configure_construction_fixture(commands, q_familiars, world_map, config.size)
        }
        PerfWorkload::UiGpu => {
            configure_ui_gpu_fixture(commands, q_familiars, world_map, config.size)
        }
    }
}

#[cfg(feature = "profiling")]
fn configure_gather_baseline(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_trees: &Query<Entity, With<Tree>>,
    q_rocks: &Query<Entity, With<Rock>>,
) {
    let area = TaskArea::from_points(Vec2::new(-1600.0, -1600.0), Vec2::new(1600.0, 1600.0));

    for (fam_entity, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        operation.max_controlled_soul = 20;
        commands.entity(fam_entity).insert(area.clone());
    }

    for tree_entity in q_trees.iter() {
        commands.entity(tree_entity).insert((
            Designation {
                work_type: WorkType::Chop,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
    }

    for rock_entity in q_rocks.iter() {
        commands.entity(rock_entity).insert((
            Designation {
                work_type: WorkType::Mine,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
    }
}

#[cfg(feature = "profiling")]
fn configure_path_door_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_souls: &mut Query<(
        Entity,
        &mut Transform,
        &mut Destination,
        &mut Path,
        &mut AssignedTask,
    )>,
    world_map: &mut WorldMapWrite,
) -> bool {
    let Some((left_grid, door_grid, right_grid)) = find_fixture_corridor(world_map.as_ref()) else {
        error!("PERF_CAPTURE: path-door fixture could not find a free three-tile corridor");
        return false;
    };

    for (_, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
    }

    let mut soul_entities = q_souls
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<Vec<_>>();
    soul_entities.sort_unstable_by_key(|entity| entity.to_bits());
    for (ordinal, soul_entity) in soul_entities.into_iter().enumerate() {
        let Ok((_, mut transform, mut destination, mut path, mut task)) =
            q_souls.get_mut(soul_entity)
        else {
            continue;
        };
        let grid = if ordinal % 2 == 0 {
            left_grid
        } else {
            right_grid
        };
        let target = if ordinal % 2 == 0 {
            right_grid
        } else {
            left_grid
        };
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        transform.translation = position.extend(transform.translation.z);
        destination.0 = WorldMap::grid_to_world(target.0, target.1);
        path.waypoints.clear();
        path.current_index = 0;
        path.planned_destination = None;
        *task = AssignedTask::None;
    }

    let door_entity = commands
        .spawn((
            Door::default(),
            Sprite {
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(
                WorldMap::grid_to_world(door_grid.0, door_grid.1).extend(Z_MAP + 0.1),
            ),
            PerfFixtureMarker {
                kind: PerfFixtureKind::Door,
                ordinal: 0,
            },
            Name::new("PerfPathDoorFixture"),
        ))
        .id();
    world_map.register_door(door_grid, door_entity, DoorState::Closed);
    true
}

#[cfg(feature = "profiling")]
fn configure_construction_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    world_map: &mut WorldMapWrite,
    size: PerfScenarioSize,
) -> bool {
    let tile_count = match size {
        PerfScenarioSize::Small => 16,
        PerfScenarioSize::Medium => 64,
        PerfScenarioSize::Large => 128,
    };
    let mut grids = fixture_free_grids(world_map.as_ref(), tile_count);
    if grids.len() != tile_count {
        error!(
            "PERF_CAPTURE: construction fixture found only {} of {tile_count} free walkable tiles",
            grids.len()
        );
        return false;
    }
    grids.sort_unstable();
    for (_, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
    }

    let world_positions = grids
        .iter()
        .map(|(gx, gy)| WorldMap::grid_to_world(*gx, *gy))
        .collect::<Vec<_>>();
    let min = world_positions
        .iter()
        .copied()
        .reduce(Vec2::min)
        .expect("non-empty construction fixture");
    let max = world_positions
        .iter()
        .copied()
        .reduce(Vec2::max)
        .expect("non-empty construction fixture");
    let position = (min + max) * 0.5;
    let area = TaskArea::from_points(
        min - Vec2::splat(TILE_SIZE * 0.5),
        max + Vec2::splat(TILE_SIZE * 0.5),
    );
    let mut site = FloorConstructionSite::new(area, position, tile_count as u32);
    site.phase = FloorConstructionPhase::Curing;
    site.tiles_reinforced = tile_count as u32;
    site.tiles_poured = tile_count as u32;
    site.curing_remaining_secs = 300.0;
    let site_entity = commands
        .spawn((
            site,
            Transform::from_translation(position.extend(Z_MAP)),
            PerfFixtureMarker {
                kind: PerfFixtureKind::ConstructionSite,
                ordinal: 0,
            },
            Name::new("PerfConstructionSiteFixture"),
        ))
        .id();
    for (ordinal, grid) in grids.into_iter().enumerate() {
        let tile_position = WorldMap::grid_to_world(grid.0, grid.1);
        let mut tile = FloorTileBlueprint::new(site_entity, grid);
        tile.state = FloorTileState::Complete;
        commands.spawn((
            tile,
            Transform::from_translation(tile_position.extend(Z_MAP)),
            PerfFixtureMarker {
                kind: PerfFixtureKind::ConstructionTile,
                ordinal: ordinal as u32,
            },
            Name::new("PerfConstructionTileFixture"),
        ));
    }
    true
}

#[cfg(feature = "profiling")]
fn configure_ui_gpu_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    world_map: &mut WorldMapWrite,
    size: PerfScenarioSize,
) -> bool {
    for (_, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
    }

    let count = match size {
        PerfScenarioSize::Small => 64,
        PerfScenarioSize::Medium => 160,
        PerfScenarioSize::Large => 320,
    };
    let mut grids = fixture_free_grids(world_map.as_ref(), count);
    if grids.len() != count {
        error!(
            "PERF_CAPTURE: ui-gpu fixture found only {} of {count} free walkable tiles",
            grids.len()
        );
        return false;
    }
    grids.sort_unstable();
    for (ordinal, grid) in grids.into_iter().enumerate() {
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        commands.spawn((
            Blueprint::new(BuildingType::Wall, vec![grid]),
            BlueprintVisualState {
                progress: 0.5,
                ..default()
            },
            Sprite {
                color: Color::srgba(0.85, 0.9, 1.0, 1.0),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(position.extend(Z_MAP + 0.2)),
            PerfFixtureMarker {
                kind: PerfFixtureKind::UiBlueprint,
                ordinal: ordinal as u32,
            },
            Name::new("PerfUiGpuBlueprintFixture"),
        ));
    }
    true
}

#[cfg(feature = "profiling")]
type PerfGridPosition = (i32, i32);
#[cfg(feature = "profiling")]
type PerfFixtureCorridor = (PerfGridPosition, PerfGridPosition, PerfGridPosition);

#[cfg(feature = "profiling")]
fn find_fixture_corridor(world_map: &WorldMap) -> Option<PerfFixtureCorridor> {
    for y in 1..MAP_HEIGHT.saturating_sub(1) {
        for x in 2..MAP_WIDTH.saturating_sub(2) {
            let grids = [(x - 1, y), (x, y), (x + 1, y)];
            if grids
                .iter()
                .all(|&(gx, gy)| fixture_grid_is_free(world_map, (gx, gy)))
            {
                return Some((grids[0], grids[1], grids[2]));
            }
        }
    }
    None
}

#[cfg(feature = "profiling")]
fn fixture_free_grids(world_map: &WorldMap, count: usize) -> Vec<(i32, i32)> {
    let mut grids = Vec::with_capacity(count);
    for y in 1..MAP_HEIGHT.saturating_sub(1) {
        for x in 1..MAP_WIDTH.saturating_sub(1) {
            let grid = (x, y);
            if fixture_grid_is_free(world_map, grid) {
                grids.push(grid);
                if grids.len() == count {
                    return grids;
                }
            }
        }
    }
    grids
}

#[cfg(feature = "profiling")]
fn fixture_grid_is_free(world_map: &WorldMap, grid: (i32, i32)) -> bool {
    world_map.is_walkable(grid.0, grid.1)
        && !world_map.buildings.contains_key(&grid)
        && !world_map.doors.contains_key(&grid)
}
