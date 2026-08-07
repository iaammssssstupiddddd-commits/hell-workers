use super::*;

#[cfg(feature = "profiling")]
use hw_core::relationships::{CommandedBy, ManagedBy, ParkedAt};
#[cfg(feature = "profiling")]
use hw_logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, ReceiverPolicyTier, TransportDemand,
    TransportPolicy, TransportPriority, TransportRequest, TransportRequestFixedSource,
    TransportRequestKind, TransportRequestState,
};
#[cfg(feature = "profiling")]
use hw_logistics::{ResourceItem, ResourceType, Stockpile, StockpilePolicy, Wheelbarrow};
#[cfg(feature = "profiling")]
use hw_ui::components::{LeftPanelMode, OperationDialog, OperationDialogState};
#[cfg(feature = "profiling")]
use hw_ui::panels::task_list::{TaskDashboardViewState, TaskListDirty, TaskWorkTypeFilter};

#[cfg(feature = "profiling")]
#[derive(Resource, Default)]
pub(crate) struct PerfScenarioApplied {
    pub(super) workload: bool,
    ui_mode: bool,
}

#[cfg(feature = "profiling")]
impl PerfScenarioApplied {
    pub(crate) const fn complete(&self) -> bool {
        self.workload && self.ui_mode
    }
}

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
    DashboardStockpile,
    DashboardResource,
    DashboardWheelbarrow,
    DashboardTransportRequest,
    DashboardDesignation,
}

#[cfg(feature = "profiling")]
impl PerfFixtureKind {
    pub(super) const fn audit_tag(self) -> u8 {
        match self {
            Self::Door => 0,
            Self::ConstructionSite => 1,
            Self::ConstructionTile => 2,
            Self::UiBlueprint => 3,
            Self::DashboardStockpile => 4,
            Self::DashboardResource => 5,
            Self::DashboardWheelbarrow => 6,
            Self::DashboardTransportRequest => 7,
            Self::DashboardDesignation => 8,
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
pub(super) type PerfSetupFamiliarFilter = (With<Familiar>, Without<DamnedSoul>);
#[cfg(feature = "profiling")]
pub(super) type PerfSetupSoulFilter = (With<DamnedSoul>, Without<Familiar>);
#[cfg(feature = "profiling")]
pub(super) type PerfSetupFamiliarQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut ActiveCommand,
        &'static mut FamiliarOperation,
        &'static mut FamiliarPolicy,
    ),
    PerfSetupFamiliarFilter,
>;
#[cfg(feature = "profiling")]
pub(super) type PerfSetupSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static mut AssignedTask,
    ),
    PerfSetupSoulFilter,
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
    q_yards: Query<'w, 's, &'static hw_world::Yard>,
    world_map: WorldMapWrite<'w>,
    game_assets: Res<'w, crate::assets::GameAssets>,
    handles_3d: Res<'w, crate::plugins::startup::Building3dHandles>,
    settings: ResMut<'w, hw_core::GameSettings>,
    indoor_light: ResMut<'w, super::indoor_light_fixture::IndoorLightFixtureState>,
    exit: MessageWriter<'w, AppExit>,
}

#[cfg(feature = "profiling")]
#[derive(SystemParam)]
pub struct PerfUiModeSetupParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    applied: ResMut<'w, PerfScenarioApplied>,
    dialog_state: ResMut<'w, OperationDialogState>,
    left_panel_mode: ResMut<'w, LeftPanelMode>,
    dashboard_view_state: ResMut<'w, TaskDashboardViewState>,
    task_list_dirty: ResMut<'w, TaskListDirty>,
    q_familiars: Query<'w, 's, Entity, With<Familiar>>,
    q_dialog: Query<'w, 's, &'static mut Node, With<OperationDialog>>,
}

#[cfg(feature = "profiling")]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PerfScenarioSet {
    FixtureSpawn,
    FixtureApply,
    Setup,
    Apply,
    IndoorSettle,
    FixtureSustain,
    UiSetup,
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
        q_yards,
        mut world_map,
        game_assets,
        handles_3d,
        mut settings,
        mut indoor_light,
        mut exit,
    } = params;

    if applied.workload || !config.enabled() || q_familiars.is_empty() {
        return;
    }

    if config.workload == PerfWorkload::IndoorLight {
        settings.power_priority_enabled = true;
        super::indoor_light_fixture::begin_indoor_light_fixture(
            &config,
            super::indoor_light_fixture::IndoorLightFixtureSetupContext {
                commands: &mut commands,
                state: &mut indoor_light,
                q_familiars: &mut q_familiars,
                q_souls: &mut q_souls,
                world_map: &mut world_map,
                q_existing_yards: &q_yards,
                game_assets: &game_assets,
                handles_3d: &handles_3d,
                exit: &mut exit,
            },
        );
        return;
    }

    applied.workload = configure_perf_workload(
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
pub fn setup_perf_ui_mode_if_enabled(params: PerfUiModeSetupParams) {
    let PerfUiModeSetupParams {
        config,
        mut applied,
        mut dialog_state,
        mut left_panel_mode,
        mut dashboard_view_state,
        mut task_list_dirty,
        q_familiars,
        mut q_dialog,
    } = params;
    if applied.ui_mode || !config.enabled() || !applied.workload {
        return;
    }
    let Ok(mut dialog) = q_dialog.single_mut() else {
        return;
    };

    match config.operation_dialog_mode {
        PerfOperationDialogMode::Hidden => {
            dialog_state.target = None;
            dialog.display = Display::None;
        }
        PerfOperationDialogMode::Open => {
            let Some(target) = q_familiars.iter().min_by_key(|entity| entity.to_bits()) else {
                return;
            };
            dialog_state.target = Some(target);
            dialog.display = Display::Flex;
        }
    }

    *dashboard_view_state = TaskDashboardViewState::default();
    *left_panel_mode = match config.dashboard_mode {
        PerfDashboardMode::Hidden => LeftPanelMode::EntityList,
        PerfDashboardMode::Visible | PerfDashboardMode::ActiveFilter => LeftPanelMode::TaskList,
    };
    if matches!(config.dashboard_mode, PerfDashboardMode::ActiveFilter) {
        dashboard_view_state.work_type = TaskWorkTypeFilter::Only(WorkType::Chop);
    }
    task_list_dirty.mark_all();
    applied.ui_mode = true;
}

#[cfg(feature = "profiling")]
fn configure_perf_workload(
    config: &PerfScenarioConfig,
    commands: &mut Commands,
    q_familiars: &mut Query<
        (
            Entity,
            &Transform,
            &mut ActiveCommand,
            &mut FamiliarOperation,
            &mut FamiliarPolicy,
        ),
        PerfSetupFamiliarFilter,
    >,
    q_souls: &mut Query<
        (
            Entity,
            &mut Transform,
            &mut Destination,
            &mut Path,
            &mut AssignedTask,
        ),
        PerfSetupSoulFilter,
    >,
    q_trees: &Query<Entity, With<Tree>>,
    q_rocks: &Query<Entity, With<Rock>>,
    world_map: &mut WorldMapWrite,
) -> bool {
    match config.workload {
        PerfWorkload::Gather => {
            configure_gather_baseline(config, commands, q_familiars, q_souls, q_trees, q_rocks);
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
        PerfWorkload::TaskDashboard => {
            configure_task_dashboard_fixture(commands, q_familiars, config.size)
        }
        PerfWorkload::IndoorLight => {
            unreachable!("indoor-light uses the production-topology settle pipeline")
        }
    }
}

#[cfg(feature = "profiling")]
fn configure_task_dashboard_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<
        (
            Entity,
            &Transform,
            &mut ActiveCommand,
            &mut FamiliarOperation,
            &mut FamiliarPolicy,
        ),
        PerfSetupFamiliarFilter,
    >,
    size: PerfScenarioSize,
) -> bool {
    let per_work_type = match size {
        PerfScenarioSize::Small => 32,
        PerfScenarioSize::Medium => 80,
        PerfScenarioSize::Large => 160,
    };
    let area = TaskArea::from_points(Vec2::new(-1600.0, -1600.0), Vec2::new(1600.0, 1600.0));
    let mut familiar_positions = Vec::new();
    for (entity, transform, mut command, mut operation, _) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        operation.max_controlled_soul = 20;
        commands.entity(entity).insert(area.clone());
        familiar_positions.push((entity, transform.translation.truncate()));
    }
    familiar_positions.sort_unstable_by_key(|(entity, _)| entity.to_bits());

    let Some((owner, position)) = familiar_positions.first().copied() else {
        return false;
    };
    for ordinal in 0..per_work_type {
        let offset = Vec2::new(
            ((ordinal % 16) as f32 - 8.0) * 8.0,
            ((ordinal / 16) as f32 - 5.0) * 8.0,
        );
        let priority = Priority([0, 5, 10][ordinal % 3]);
        commands.spawn((
            Name::new("PerfTaskDashboardChopDesignation"),
            Tree,
            Transform::from_translation((position + offset).extend(Z_MAP)),
            Designation {
                work_type: WorkType::Chop,
            },
            TaskSlots::new(1),
            priority,
            PerfFixtureMarker {
                kind: PerfFixtureKind::DashboardDesignation,
                ordinal: ordinal as u32,
            },
        ));
        commands.spawn((
            Name::new("PerfTaskDashboardMineDesignation"),
            Rock,
            Transform::from_translation((position - offset).extend(Z_MAP)),
            Designation {
                work_type: WorkType::Mine,
            },
            TaskSlots::new(1),
            priority,
            PerfFixtureMarker {
                kind: PerfFixtureKind::DashboardDesignation,
                ordinal: (per_work_type + ordinal) as u32,
            },
        ));
    }

    configure_task_dashboard_transport_fixture(commands, owner, position);
    true
}

#[cfg(feature = "profiling")]
fn configure_task_dashboard_transport_fixture(
    commands: &mut Commands,
    owner: Entity,
    position: Vec2,
) {
    let stockpile = commands
        .spawn((
            Name::new("PerfTaskDashboardStockpile"),
            Transform::from_translation(position.extend(Z_MAP)),
            Stockpile {
                capacity: 16,
                resource_type: None,
            },
            StockpilePolicy::for_capacity(16),
            PerfFixtureMarker {
                kind: PerfFixtureKind::DashboardStockpile,
                ordinal: 0,
            },
        ))
        .id();
    for ordinal in 0..3 {
        commands.spawn((
            Name::new("PerfTaskDashboardArbitrationSource"),
            Transform::from_translation(
                (position + Vec2::new(4.0 + ordinal as f32, 0.0)).extend(Z_MAP),
            ),
            Visibility::Visible,
            ResourceItem(ResourceType::Rock),
            PerfFixtureMarker {
                kind: PerfFixtureKind::DashboardResource,
                ordinal,
            },
        ));
    }
    // Keep the blueprint just inside the familiar TaskArea so the production
    // blueprint producer maintains its request, while the source remains just
    // outside the area so normal haul selection cannot designate it first. Sand
    // is intentional: DeliverToBlueprint participates in wheelbarrow arbitration
    // only for resources that require a wheelbarrow.
    let blueprint_position = Vec2::new(1550.0, 0.0);
    let direct_source_position = Vec2::new(1650.0, 0.0);
    commands.spawn((
        Name::new("PerfTaskDashboardDirectSource"),
        Transform::from_translation(direct_source_position.extend(Z_MAP)),
        Visibility::Visible,
        ResourceItem(ResourceType::Sand),
        PerfFixtureMarker {
            kind: PerfFixtureKind::DashboardResource,
            ordinal: 3,
        },
    ));
    let parking = commands
        .spawn((
            Name::new("PerfTaskDashboardWheelbarrowParking"),
            Transform::from_translation((position + Vec2::new(-2.0, 0.0)).extend(Z_MAP)),
        ))
        .id();
    commands.spawn((
        Name::new("PerfTaskDashboardWheelbarrow"),
        Transform::from_translation((position + Vec2::new(-1.0, 0.0)).extend(Z_MAP)),
        Wheelbarrow { capacity: 10 },
        ParkedAt(parking),
        PerfFixtureMarker {
            kind: PerfFixtureKind::DashboardWheelbarrow,
            ordinal: 0,
        },
    ));
    commands.spawn((
        Name::new("PerfTaskDashboardArbitrationRequest"),
        Transform::from_translation(position.extend(Z_MAP)),
        Visibility::Hidden,
        Designation {
            work_type: WorkType::Haul,
        },
        ManagedBy(owner),
        TaskSlots::new(1),
        Priority(10),
        ReceiverPolicyTier(TransportPriority::Normal),
        TransportRequest {
            kind: TransportRequestKind::DepositToStockpile,
            anchor: stockpile,
            resource_type: ResourceType::Rock,
            issued_by: owner,
            priority: TransportPriority::Normal,
            stockpile_group: vec![stockpile],
        },
        TransportDemand {
            desired_slots: 1,
            inflight: 0,
        },
        TransportPolicy::default(),
        TransportRequestState::Pending,
        PerfFixtureMarker {
            kind: PerfFixtureKind::DashboardTransportRequest,
            ordinal: 0,
        },
    ));
    let mut blueprint_fixture = Blueprint::new(BuildingType::Wall, vec![]);
    blueprint_fixture.required_materials.clear();
    blueprint_fixture
        .required_materials
        .insert(ResourceType::Sand, 1);
    let blueprint = commands
        .spawn((
            Name::new("PerfTaskDashboardBlueprint"),
            blueprint_fixture,
            Transform::from_translation(blueprint_position.extend(Z_MAP)),
            Designation {
                work_type: WorkType::Build,
            },
            TaskSlots::new(1),
            Priority(5),
            PerfFixtureMarker {
                kind: PerfFixtureKind::UiBlueprint,
                ordinal: 0,
            },
        ))
        .id();
    commands.spawn((
        Name::new("PerfTaskDashboardDirectHaulRequest"),
        Transform::from_translation(blueprint_position.extend(Z_MAP)),
        Visibility::Hidden,
        Designation {
            work_type: WorkType::Haul,
        },
        ManagedBy(owner),
        TaskSlots::new(1),
        Priority(5),
        TargetBlueprint(blueprint),
        TransportRequest {
            kind: TransportRequestKind::DeliverToBlueprint,
            anchor: blueprint,
            resource_type: ResourceType::Sand,
            issued_by: owner,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots: 1,
            inflight: 0,
        },
        TransportPolicy::default(),
        TransportRequestState::Pending,
        PerfFixtureMarker {
            kind: PerfFixtureKind::DashboardTransportRequest,
            ordinal: 1,
        },
    ));
}

#[cfg(feature = "profiling")]
fn configure_gather_baseline(
    config: &PerfScenarioConfig,
    commands: &mut Commands,
    q_familiars: &mut Query<
        (
            Entity,
            &Transform,
            &mut ActiveCommand,
            &mut FamiliarOperation,
            &mut FamiliarPolicy,
        ),
        PerfSetupFamiliarFilter,
    >,
    q_souls: &mut Query<
        (
            Entity,
            &mut Transform,
            &mut Destination,
            &mut Path,
            &mut AssignedTask,
        ),
        PerfSetupSoulFilter,
    >,
    q_trees: &Query<Entity, With<Tree>>,
    q_rocks: &Query<Entity, With<Rock>>,
) {
    let area = TaskArea::from_points(Vec2::new(-1600.0, -1600.0), Vec2::new(1600.0, 1600.0));
    let mut familiar_positions = Vec::new();

    for (fam_entity, transform, mut command, mut operation, mut policy) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        if config.familiar_policy_mode.uses_controlled_fixture() {
            let policy = policy.bypass_change_detection();
            *policy = FamiliarPolicy::default();
            if matches!(
                config.familiar_policy_mode,
                PerfFamiliarPolicyMode::Disabled
            ) {
                policy.set_all_allowed(false);
            }
            familiar_positions.push((fam_entity, transform.translation.truncate()));
        } else {
            operation.max_controlled_soul = 20;
        }
        commands.entity(fam_entity).insert(area.clone());
    }
    familiar_positions.sort_unstable_by_key(|(entity, _)| entity.to_bits());

    if config.familiar_policy_mode.uses_controlled_fixture() {
        if let Some(tree_entity) = q_trees.iter().min_by_key(|entity| entity.to_bits()) {
            commands.entity(tree_entity).insert((
                Designation {
                    work_type: WorkType::Chop,
                },
                TaskSlots::new(1),
                Priority(0),
            ));
        }
    } else {
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

    if config.familiar_policy_mode.uses_controlled_fixture() {
        configure_controlled_familiar_policy_fixture(commands, q_souls, &familiar_positions);
    }
}

#[cfg(feature = "profiling")]
fn configure_controlled_familiar_policy_fixture(
    commands: &mut Commands,
    q_souls: &mut Query<
        (
            Entity,
            &mut Transform,
            &mut Destination,
            &mut Path,
            &mut AssignedTask,
        ),
        PerfSetupSoulFilter,
    >,
    familiar_positions: &[(Entity, Vec2)],
) {
    let Some((owner, fixture_pos)) = familiar_positions.first().copied() else {
        return;
    };
    let mut souls = q_souls
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<Vec<_>>();
    souls.sort_unstable_by_key(|entity| entity.to_bits());
    if souls.len() < 2 {
        return;
    }
    for soul in souls.into_iter().take(2) {
        if let Ok((_, mut transform, mut destination, mut path, mut task)) = q_souls.get_mut(soul) {
            transform.translation.x = fixture_pos.x;
            transform.translation.y = fixture_pos.y;
            destination.0 = fixture_pos;
            path.waypoints.clear();
            path.current_index = 0;
            path.planned_destination = None;
            *task = AssignedTask::None;
            commands.entity(soul).insert(CommandedBy(owner));
        }
    }

    let stockpile = commands
        .spawn((
            Name::new("PerfFamiliarPolicyStockpile"),
            Transform::from_translation(fixture_pos.extend(Z_MAP)),
            Stockpile {
                capacity: 8,
                resource_type: None,
            },
            StockpilePolicy::for_capacity(8),
        ))
        .id();
    let fixed_source = commands
        .spawn((
            Name::new("PerfFamiliarPolicySource"),
            Transform::from_translation((fixture_pos + Vec2::new(4.0, 0.0)).extend(Z_MAP)),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
            ManualHaulPinnedSource,
        ))
        .id();
    commands.spawn((
        Name::new("PerfFamiliarPolicyRequest"),
        Transform::from_translation(fixture_pos.extend(Z_MAP)),
        Visibility::Hidden,
        Designation {
            work_type: WorkType::Haul,
        },
        ManagedBy(owner),
        ManualTransportRequest,
        TransportRequestFixedSource(fixed_source),
        TaskSlots::new(1),
        Priority(10),
        ReceiverPolicyTier(TransportPriority::Normal),
        TransportRequest {
            kind: TransportRequestKind::DepositToStockpile,
            anchor: stockpile,
            resource_type: ResourceType::Wood,
            issued_by: owner,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots: 1,
            inflight: 0,
        },
        TransportPolicy::default(),
        TransportRequestState::Pending,
    ));
}

#[cfg(feature = "profiling")]
fn configure_path_door_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<
        (
            Entity,
            &Transform,
            &mut ActiveCommand,
            &mut FamiliarOperation,
            &mut FamiliarPolicy,
        ),
        PerfSetupFamiliarFilter,
    >,
    q_souls: &mut Query<
        (
            Entity,
            &mut Transform,
            &mut Destination,
            &mut Path,
            &mut AssignedTask,
        ),
        PerfSetupSoulFilter,
    >,
    world_map: &mut WorldMapWrite,
) -> bool {
    let Some((left_grid, door_grid, right_grid)) = find_fixture_corridor(world_map.as_ref()) else {
        error!("PERF_CAPTURE: path-door fixture could not find a free three-tile corridor");
        return false;
    };

    for (_, _, mut command, mut operation, _) in q_familiars.iter_mut() {
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
    q_familiars: &mut Query<
        (
            Entity,
            &Transform,
            &mut ActiveCommand,
            &mut FamiliarOperation,
            &mut FamiliarPolicy,
        ),
        PerfSetupFamiliarFilter,
    >,
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
    for (_, _, mut command, mut operation, _) in q_familiars.iter_mut() {
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
    q_familiars: &mut Query<
        (
            Entity,
            &Transform,
            &mut ActiveCommand,
            &mut FamiliarOperation,
            &mut FamiliarPolicy,
        ),
        PerfSetupFamiliarFilter,
    >,
    world_map: &mut WorldMapWrite,
    size: PerfScenarioSize,
) -> bool {
    for (_, _, mut command, mut operation, _) in q_familiars.iter_mut() {
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
