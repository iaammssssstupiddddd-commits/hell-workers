use std::collections::{BTreeMap, HashSet};

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, Z_AURA};
use hw_core::visual_mirror::construction::BlueprintVisualState;
use hw_infra::lighting::digest_hex;
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_ui::camera::MainCamera;
use hw_visual::visual3d::Building3dVisual;
use hw_world::{WorldMap, WorldMapRead, WorldMapWrite};
use sha2::{Digest, Sha256};

use super::fixture::{
    PerfFixtureKind, PerfFixtureMarker, PerfMainCameraQuery, PerfScenarioApplied,
};
use super::{PerfScenarioConfig, PerfScenarioSize, PerfWallPhase, PerfWorkload};
use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::wall_construction::spawn_wall_shell;
use crate::systems::jobs::{Designation, TaskSlots, WorkType};

pub(super) const CONTRACT_ID: &str = "wall-density-v1";
pub(super) const CONTRACT_BYTES: &[u8] =
    include_bytes!("../../../../../../tools/blender_ai_workflow/fixtures/wall-density-v1.json");
pub(super) const CONTRACT_SHA256: &str =
    "7b32f4e0ecd9cdb9223cde1b7f5aae93460e3ec861b2e3ae0e1eb2e2b87419c8";
pub(super) const FORMWORK_CONTRACT_ID: &str = "wall-formwork-density-v1";
pub(super) const FORMWORK_CONTRACT_BYTES: &[u8] = include_bytes!(
    "../../../../../../tools/blender_ai_workflow/fixtures/wall-formwork-density-v1.json"
);
pub(super) const FORMWORK_CONTRACT_SHA256: &str =
    "fe3c9d2c953f08af5cbc9bedefbbdbfd5074c130f4aa76577679b50926998115";

const GRID_ORIGIN: (i32, i32) = (2, 2);
const GRID_STRIDE: (i32, i32) = (5, 5);
const GRID_COLUMNS: usize = 20;
pub(super) const CAMERA_SCALE: f32 = 5.0;
pub(super) const ACTUAL_WINDOW_SUBJECT_ORDINAL: u32 = 64;
const COMPLETED_SOUL_DEPTH_SUBJECT_ORDINAL: u32 = 84;
const MASK_COUNT: usize = 16;
const MIXED_GALLERY_PAIR_GRIDS: [(i32, i32); 2] = [(20, 15), (21, 15)];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum WallDensityFixturePhase {
    #[default]
    Inactive,
    Spawned,
    Ready,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConnectorSpec {
    grid: (i32, i32),
    direction: &'static str,
}

#[derive(Clone, Debug)]
struct SpecimenSpec {
    ordinal: u32,
    grid: (i32, i32),
    mask: u8,
    wall: Option<Entity>,
    connectors: Vec<(ConnectorSpec, Option<Entity>)>,
}

#[derive(Clone, Debug)]
struct WallDensityLayout {
    size: PerfScenarioSize,
    phase: PerfWallPhase,
    specimens: Vec<SpecimenSpec>,
    layout_checksum: String,
}

#[derive(Clone, Copy, Debug)]
struct MixedGalleryPair {
    provisional: Entity,
    completed: Entity,
}

impl WallDensityLayout {
    fn build(size: PerfScenarioSize, phase: PerfWallPhase) -> Self {
        let target_count = target_count(size);
        let mut specimens = Vec::with_capacity(target_count);
        for ordinal in 0..target_count {
            let column = ordinal % GRID_COLUMNS;
            let row = ordinal / GRID_COLUMNS;
            let grid = (
                GRID_ORIGIN.0
                    + i32::try_from(column).expect("wall fixture column fits i32") * GRID_STRIDE.0,
                GRID_ORIGIN.1
                    + i32::try_from(row).expect("wall fixture row fits i32") * GRID_STRIDE.1,
            );
            let mask = u8::try_from(ordinal % MASK_COUNT).expect("wall fixture mask fits u8");
            let connectors = connector_specs(grid, mask)
                .into_iter()
                .map(|connector| (connector, None))
                .collect();
            specimens.push(SpecimenSpec {
                ordinal: u32::try_from(ordinal).expect("wall fixture ordinal fits u32"),
                grid,
                mask,
                wall: None,
                connectors,
            });
        }
        let layout_checksum = calculate_layout_checksum(size, phase, &specimens);
        Self {
            size,
            phase,
            specimens,
            layout_checksum,
        }
    }

    fn connector_count(&self) -> usize {
        self.specimens
            .iter()
            .map(|specimen| specimen.connectors.len())
            .sum()
    }

    fn mask_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for specimen in &self.specimens {
            *counts.entry(format!("{:04b}", specimen.mask)).or_default() += 1;
        }
        counts
    }

    fn is_provisional(&self, ordinal: u32) -> bool {
        is_provisional_phase(self.phase, ordinal)
    }

    fn phase_counts(&self) -> (usize, usize) {
        let provisional = self
            .specimens
            .iter()
            .filter(|specimen| self.is_provisional(specimen.ordinal))
            .count();
        (self.specimens.len() - provisional, provisional)
    }

    fn phase_mask_counts(&self, provisional: bool) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for specimen in &self.specimens {
            if self.is_provisional(specimen.ordinal) == provisional {
                *counts.entry(format!("{:04b}", specimen.mask)).or_default() += 1;
            }
        }
        counts
    }
}

#[derive(Resource, Default)]
pub(crate) struct WallDensityFixtureState {
    pub(super) phase: WallDensityFixturePhase,
    layout: Option<WallDensityLayout>,
    mixed_gallery_pair: Option<MixedGalleryPair>,
    pub(super) failure: Option<String>,
}

pub(super) struct WallDensityProbeSubject<'a> {
    pub(super) entity: Entity,
    pub(super) ordinal: u32,
    pub(super) grid: (i32, i32),
    pub(super) mask: u8,
    pub(super) layout_checksum: &'a str,
    pub(super) phase: PerfWallPhase,
}

type GalleryPhasePair = [(Entity, (i32, i32), bool); 2];

pub(super) struct WallDensityRenderDocEvidence {
    pub(super) target_entities: Vec<Entity>,
    pub(super) mixed_gallery_pair: Option<GalleryPhasePair>,
    pub(super) layout_checksum: String,
    pub(super) phase: PerfWallPhase,
    pub(super) target_wall_count: usize,
    pub(super) completed_wall_count: usize,
    pub(super) provisional_wall_count: usize,
    pub(super) connector_count: usize,
    pub(super) mask_counts: BTreeMap<String, usize>,
    pub(super) completed_mask_counts: BTreeMap<String, usize>,
    pub(super) provisional_mask_counts: BTreeMap<String, usize>,
}

impl WallDensityFixtureState {
    pub(super) fn mixed_gallery_pair(&self) -> Option<[(Entity, (i32, i32)); 2]> {
        let pair = self.mixed_gallery_pair?;
        Some([
            (pair.provisional, MIXED_GALLERY_PAIR_GRIDS[0]),
            (pair.completed, MIXED_GALLERY_PAIR_GRIDS[1]),
        ])
    }

    pub(super) fn actual_window_subject(&self) -> Option<WallDensityProbeSubject<'_>> {
        self.probe_subject(|specimen| specimen.ordinal == ACTUAL_WINDOW_SUBJECT_ORDINAL)
    }

    /// Lowest-ordinal specimen carrying `mask`, used to sample one straight run
    /// across its wall body.
    pub(super) fn mask_subject(&self, mask: u8) -> Option<WallDensityProbeSubject<'_>> {
        self.probe_subject(|specimen| specimen.mask == mask)
    }

    pub(super) fn soul_depth_subjects(&self) -> Option<GalleryPhasePair> {
        let layout = self.layout.as_ref().filter(|layout| {
            self.phase == WallDensityFixturePhase::Ready && layout.phase == PerfWallPhase::Mixed
        })?;
        let subject = |ordinal| {
            let specimen = layout
                .specimens
                .iter()
                .find(|specimen| specimen.ordinal == ordinal)?;
            Some((
                specimen.wall?,
                specimen.grid,
                layout.is_provisional(specimen.ordinal),
            ))
        };
        let provisional = subject(ACTUAL_WINDOW_SUBJECT_ORDINAL)?;
        let completed = subject(COMPLETED_SOUL_DEPTH_SUBJECT_ORDINAL)?;
        (provisional.2 && !completed.2).then_some([provisional, completed])
    }

    fn probe_subject(
        &self,
        select: impl Fn(&SpecimenSpec) -> bool,
    ) -> Option<WallDensityProbeSubject<'_>> {
        let layout = self
            .layout
            .as_ref()
            .filter(|_| self.phase == WallDensityFixturePhase::Ready)?;
        let specimen = layout.specimens.iter().find(|specimen| select(specimen))?;
        Some(WallDensityProbeSubject {
            entity: specimen.wall?,
            ordinal: specimen.ordinal,
            grid: specimen.grid,
            mask: specimen.mask,
            layout_checksum: &layout.layout_checksum,
            phase: layout.phase,
        })
    }

    pub(super) fn sidecars(&self) -> Result<(serde_json::Value, String), String> {
        if self.phase != WallDensityFixturePhase::Ready {
            return Err(format!(
                "wall-density sidecar requested in {:?} phase",
                self.phase
            ));
        }
        let layout = self
            .layout
            .as_ref()
            .ok_or_else(|| "wall-density Ready state has no layout".to_string())?;
        let (contract_id, contract_bytes, expected_contract_sha256) =
            contract_for_phase(layout.phase);
        let contract_sha256 = contract_sha256(contract_bytes);
        if contract_sha256 != expected_contract_sha256 {
            return Err(format!(
                "wall-density embedded contract hash changed: {contract_sha256}"
            ));
        }
        let summary = serde_json::json!({
            "schema_version": 1,
            "contract_id": contract_id,
            "contract_sha256": contract_sha256,
            "layout_checksum": layout.layout_checksum,
            "target_size": target_size_name(layout.size),
            "perf_size": layout.size.as_str(),
            "phase": layout.phase.as_str(),
            "target_wall_count": layout.specimens.len(),
            "connector_count": layout.connector_count(),
            "mask_counts": layout.mask_counts(),
            "grid": {
                "origin": [GRID_ORIGIN.0, GRID_ORIGIN.1],
                "stride": [GRID_STRIDE.0, GRID_STRIDE.1],
                "columns": GRID_COLUMNS,
            },
            "camera_scale": CAMERA_SCALE,
        });

        let mut csv = String::from(
            "schema_version,record_kind,ordinal,target_ordinal,grid_x,grid_y,mask,direction,phase\n",
        );
        let mut connector_ordinal = 0usize;
        for specimen in &layout.specimens {
            csv.push_str(&format!(
                "1,target,{},{},{},{},{:04b},,{}\n",
                specimen.ordinal,
                specimen.ordinal,
                specimen.grid.0,
                specimen.grid.1,
                specimen.mask,
                layout.phase.as_str(),
            ));
            for (connector, _) in &specimen.connectors {
                csv.push_str(&format!(
                    "1,connector,{connector_ordinal},{},{},{},{:04b},{},{}\n",
                    specimen.ordinal,
                    connector.grid.0,
                    connector.grid.1,
                    specimen.mask,
                    connector.direction,
                    layout.phase.as_str(),
                ));
                connector_ordinal += 1;
            }
        }
        Ok((summary, csv))
    }

    pub(super) fn renderdoc_evidence(&self) -> Result<WallDensityRenderDocEvidence, String> {
        if self.phase != WallDensityFixturePhase::Ready {
            return Err(format!(
                "wall-density RenderDoc evidence requested in {:?} phase",
                self.phase
            ));
        }
        let layout = self
            .layout
            .as_ref()
            .ok_or_else(|| "wall-density Ready state has no layout".to_string())?;
        let mut target_entities = layout
            .specimens
            .iter()
            .map(|specimen| {
                specimen.wall.ok_or_else(|| {
                    format!(
                        "wall-density target {} has no RenderDoc entity",
                        specimen.ordinal
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mixed_gallery_pair = self.mixed_gallery_pair.map(|pair| {
            [
                (pair.provisional, MIXED_GALLERY_PAIR_GRIDS[0], true),
                (pair.completed, MIXED_GALLERY_PAIR_GRIDS[1], false),
            ]
        });
        target_entities.extend(
            mixed_gallery_pair
                .iter()
                .flatten()
                .map(|(entity, _, _)| *entity),
        );
        let (completed_wall_count, provisional_wall_count) = layout.phase_counts();
        Ok(WallDensityRenderDocEvidence {
            target_entities,
            mixed_gallery_pair,
            layout_checksum: layout.layout_checksum.clone(),
            phase: layout.phase,
            target_wall_count: layout.specimens.len(),
            completed_wall_count,
            provisional_wall_count,
            connector_count: layout.connector_count(),
            mask_counts: layout.mask_counts(),
            completed_mask_counts: layout.phase_mask_counts(false),
            provisional_mask_counts: layout.phase_mask_counts(true),
        })
    }
}

pub(super) struct WallDensitySetupContext<'a, 'w, 's> {
    pub(super) commands: &'a mut Commands<'w, 's>,
    pub(super) state: &'a mut WallDensityFixtureState,
    pub(super) world_map: &'a mut WorldMapWrite<'w>,
    pub(super) game_assets: &'a GameAssets,
    pub(super) handles_3d: &'a Building3dHandles,
    pub(super) q_main_camera: &'a mut PerfMainCameraQuery<'w, 's>,
    pub(super) exit: &'a mut MessageWriter<'w, AppExit>,
}

pub(super) fn begin_wall_density_fixture(
    config: &PerfScenarioConfig,
    context: WallDensitySetupContext<'_, '_, '_>,
) {
    let WallDensitySetupContext {
        commands,
        state,
        world_map,
        game_assets,
        handles_3d,
        q_main_camera,
        exit,
    } = context;
    if state.phase != WallDensityFixturePhase::Inactive {
        return;
    }
    let Some(phase) = config.wall_phase() else {
        fail_fixture(state, exit, "wall-density phase is absent".to_string());
        return;
    };
    let mut layout = WallDensityLayout::build(config.size(), phase);
    if let Err(reason) = validate_layout_cells(&layout, world_map.as_ref()) {
        fail_fixture(state, exit, reason);
        return;
    }
    let Ok(mut camera_transform) = q_main_camera.single_mut() else {
        fail_fixture(
            state,
            exit,
            "wall-density requires exactly one main Camera2d".to_string(),
        );
        return;
    };
    camera_transform.translation.x = 0.0;
    camera_transform.translation.y = 0.0;
    camera_transform.scale = Vec3::new(CAMERA_SCALE, CAMERA_SCALE, 1.0);

    for specimen in &mut layout.specimens {
        let is_provisional = is_provisional_phase(phase, specimen.ordinal);
        let wall = spawn_wall_shell(commands, handles_3d, specimen.grid, is_provisional);
        commands.entity(wall).insert(PerfFixtureMarker {
            kind: PerfFixtureKind::WallDensityTarget,
            ordinal: specimen.ordinal,
        });
        world_map.reserve_building_footprint(
            BuildingType::Wall,
            wall,
            std::iter::once(specimen.grid),
        );
        specimen.wall = Some(wall);

        for (connector, entity_slot) in &mut specimen.connectors {
            let connector_entity = spawn_door_blueprint_connector(
                commands,
                world_map,
                game_assets,
                specimen.ordinal,
                *connector,
            );
            *entity_slot = Some(connector_entity);
        }
    }
    let mixed_gallery_pair = if formwork_gallery_requested(phase) {
        match spawn_mixed_gallery_pair(commands, world_map, handles_3d) {
            Ok(pair) => Some(pair),
            Err(reason) => {
                fail_fixture(state, exit, reason);
                return;
            }
        }
    } else {
        None
    };
    state.layout = Some(layout);
    state.mixed_gallery_pair = mixed_gallery_pair;
    state.failure = None;
    state.phase = WallDensityFixturePhase::Spawned;
}

fn spawn_door_blueprint_connector(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    game_assets: &GameAssets,
    target_ordinal: u32,
    connector: ConnectorSpec,
) -> Entity {
    let world_pos = WorldMap::grid_to_world(connector.grid.0, connector.grid.1);
    let (blueprint, visual_state) = door_connector_blueprint(connector.grid);
    let entity = commands
        .spawn((
            blueprint,
            visual_state,
            Designation {
                work_type: WorkType::Build,
            },
            TaskSlots::new(1),
            Sprite {
                image: game_assets.door_closed.clone(),
                color: Color::srgba(1.0, 1.0, 1.0, 0.5),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(world_pos.x, world_pos.y, Z_AURA),
            Name::new(format!(
                "PerfWallDensityDoorConnector ({target_ordinal}, {})",
                connector.direction
            )),
            PerfFixtureMarker {
                kind: PerfFixtureKind::WallDensityConnector,
                ordinal: target_ordinal,
            },
        ))
        .id();
    world_map.reserve_building_footprint(
        BuildingType::Door,
        entity,
        std::iter::once(connector.grid),
    );
    entity
}

fn formwork_gallery_requested(phase: PerfWallPhase) -> bool {
    phase == PerfWallPhase::Mixed
        && std::env::var("HW_WALL_FORMWORK_ACCEPTANCE").as_deref() == Ok("1")
}

fn spawn_mixed_gallery_pair(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    handles_3d: &Building3dHandles,
) -> Result<MixedGalleryPair, String> {
    for grid in MIXED_GALLERY_PAIR_GRIDS {
        if world_map.has_building(grid) {
            return Err(format!(
                "wall formwork gallery pair cell {grid:?} already has a logical building"
            ));
        }
    }
    let provisional = spawn_wall_shell(commands, handles_3d, MIXED_GALLERY_PAIR_GRIDS[0], true);
    world_map.reserve_building_footprint(
        BuildingType::Wall,
        provisional,
        std::iter::once(MIXED_GALLERY_PAIR_GRIDS[0]),
    );
    let completed = spawn_wall_shell(commands, handles_3d, MIXED_GALLERY_PAIR_GRIDS[1], false);
    world_map.reserve_building_footprint(
        BuildingType::Wall,
        completed,
        std::iter::once(MIXED_GALLERY_PAIR_GRIDS[1]),
    );
    Ok(MixedGalleryPair {
        provisional,
        completed,
    })
}

fn door_connector_blueprint(grid: (i32, i32)) -> (Blueprint, BlueprintVisualState) {
    let blueprint = Blueprint::new(BuildingType::Door, vec![grid]);
    let visual_state = hw_jobs::visual_sync::blueprint_visual_state(&blueprint);
    (blueprint, visual_state)
}

#[derive(SystemParam)]
pub(crate) struct WallDensityValidationParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    state: ResMut<'w, WallDensityFixtureState>,
    applied: ResMut<'w, PerfScenarioApplied>,
    world_map: WorldMapRead<'w>,
    q_walls: Query<'w, 's, (&'static Building, &'static Transform)>,
    q_visuals: Query<'w, 's, (&'static Building3dVisual, &'static Mesh3d)>,
    q_blueprints: Query<'w, 's, (&'static Blueprint, &'static Transform)>,
    q_main_camera: Query<'w, 's, &'static Transform, With<MainCamera>>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn validate_wall_density_fixture_system(mut params: WallDensityValidationParams) {
    if !params.config.enabled()
        || params.config.workload() != PerfWorkload::WallDensity
        || params.state.phase != WallDensityFixturePhase::Spawned
    {
        return;
    }
    let result = validate_live_fixture(
        params.state.layout.as_ref(),
        params.state.mixed_gallery_pair,
        params.world_map.as_ref(),
        &params.q_walls,
        &params.q_visuals,
        &params.q_blueprints,
        &params.q_main_camera,
    );
    match result {
        Ok(()) => {
            params.state.phase = WallDensityFixturePhase::Ready;
            params.applied.workload = true;
        }
        Err(reason) => fail_fixture(&mut params.state, &mut params.exit, reason),
    }
}

fn validate_live_fixture(
    layout: Option<&WallDensityLayout>,
    mixed_gallery_pair: Option<MixedGalleryPair>,
    world_map: &WorldMap,
    q_walls: &Query<(&Building, &Transform)>,
    q_visuals: &Query<(&Building3dVisual, &Mesh3d)>,
    q_blueprints: &Query<(&Blueprint, &Transform)>,
    q_main_camera: &Query<&Transform, With<MainCamera>>,
) -> Result<(), String> {
    let layout = layout.ok_or_else(|| "wall-density Spawned state has no layout".to_string())?;
    let camera = q_main_camera
        .single()
        .map_err(|_| "wall-density requires exactly one main Camera2d".to_string())?;
    if camera.translation.x != 0.0
        || camera.translation.y != 0.0
        || camera.scale != Vec3::new(CAMERA_SCALE, CAMERA_SCALE, 1.0)
    {
        return Err("wall-density camera framing changed before validation".to_string());
    }
    for specimen in &layout.specimens {
        let wall = specimen
            .wall
            .ok_or_else(|| format!("wall-density target {} has no entity", specimen.ordinal))?;
        let (building, transform) = q_walls
            .get(wall)
            .map_err(|_| format!("wall-density target {} is missing", specimen.ordinal))?;
        if building.kind != BuildingType::Wall
            || building.is_provisional != layout.is_provisional(specimen.ordinal)
            || WorldMap::world_to_grid(transform.translation.truncate()) != specimen.grid
            || world_map.building_entity(specimen.grid) != Some(wall)
        {
            return Err(format!(
                "wall-density target {} differs from its production wall contract",
                specimen.ordinal
            ));
        }
        let visual_count = q_visuals
            .iter()
            .filter(|(visual, _)| visual.owner == wall)
            .count();
        if visual_count != 1 {
            return Err(format!(
                "wall-density target {} has {visual_count} Building3dVisual owners",
                specimen.ordinal
            ));
        }
        for (connector, connector_entity) in &specimen.connectors {
            let entity = connector_entity.ok_or_else(|| {
                format!(
                    "wall-density target {} connector {} has no entity",
                    specimen.ordinal, connector.direction
                )
            })?;
            let (blueprint, transform) = q_blueprints.get(entity).map_err(|_| {
                format!(
                    "wall-density target {} connector {} is missing",
                    specimen.ordinal, connector.direction
                )
            })?;
            if blueprint.kind != BuildingType::Door
                || blueprint.occupied_grids.as_slice() != [connector.grid]
                || WorldMap::world_to_grid(transform.translation.truncate()) != connector.grid
                || world_map.building_entity(connector.grid) != Some(entity)
            {
                return Err(format!(
                    "wall-density target {} connector {} differs from its Door blueprint contract",
                    specimen.ordinal, connector.direction
                ));
            }
        }
    }
    if let Some(pair) = mixed_gallery_pair {
        for (entity, grid, is_provisional) in [
            (pair.provisional, MIXED_GALLERY_PAIR_GRIDS[0], true),
            (pair.completed, MIXED_GALLERY_PAIR_GRIDS[1], false),
        ] {
            let (building, transform) = q_walls
                .get(entity)
                .map_err(|_| "wall formwork gallery pair member is missing".to_string())?;
            if building.kind != BuildingType::Wall
                || building.is_provisional != is_provisional
                || WorldMap::world_to_grid(transform.translation.truncate()) != grid
                || world_map.building_entity(grid) != Some(entity)
                || q_visuals
                    .iter()
                    .filter(|(visual, _)| visual.owner == entity)
                    .count()
                    != 1
            {
                return Err(
                    "wall formwork gallery pair differs from its production contract".to_string(),
                );
            }
        }
    }
    Ok(())
}

fn validate_layout_cells(layout: &WallDensityLayout, world_map: &WorldMap) -> Result<(), String> {
    let mut cells = HashSet::new();
    for specimen in &layout.specimens {
        for grid in std::iter::once(specimen.grid).chain(
            specimen
                .connectors
                .iter()
                .map(|(connector, _)| connector.grid),
        ) {
            if grid.0 < 0 || grid.0 >= MAP_WIDTH || grid.1 < 0 || grid.1 >= MAP_HEIGHT {
                return Err(format!("wall-density cell {grid:?} is outside the map"));
            }
            if !cells.insert(grid) {
                return Err(format!("wall-density cell {grid:?} is used more than once"));
            }
            if world_map.has_building(grid) {
                return Err(format!(
                    "wall-density cell {grid:?} already has a logical building"
                ));
            }
        }
    }
    Ok(())
}

fn connector_specs(grid: (i32, i32), mask: u8) -> Vec<ConnectorSpec> {
    [
        (0b1000, "N", (0, 1)),
        (0b0100, "S", (0, -1)),
        (0b0010, "W", (-1, 0)),
        (0b0001, "E", (1, 0)),
    ]
    .into_iter()
    .filter_map(|(bit, direction, offset)| {
        (mask & bit != 0).then_some(ConnectorSpec {
            grid: (grid.0 + offset.0, grid.1 + offset.1),
            direction,
        })
    })
    .collect()
}

fn target_count(size: PerfScenarioSize) -> usize {
    match size {
        PerfScenarioSize::Small => 96,
        PerfScenarioSize::Medium => 384,
        PerfScenarioSize::Large => unreachable!("wall-density rejects the large perf size"),
    }
}

fn target_size_name(size: PerfScenarioSize) -> &'static str {
    match size {
        PerfScenarioSize::Small => "N",
        PerfScenarioSize::Medium => "4N",
        PerfScenarioSize::Large => unreachable!("wall-density rejects the large perf size"),
    }
}

const fn is_provisional_phase(phase: PerfWallPhase, ordinal: u32) -> bool {
    match phase {
        PerfWallPhase::Completed => false,
        PerfWallPhase::Provisional => true,
        PerfWallPhase::Mixed => (ordinal / 16).is_multiple_of(2),
    }
}

const fn contract_for_phase(phase: PerfWallPhase) -> (&'static str, &'static [u8], &'static str) {
    match phase {
        PerfWallPhase::Completed | PerfWallPhase::Provisional => {
            (CONTRACT_ID, CONTRACT_BYTES, CONTRACT_SHA256)
        }
        PerfWallPhase::Mixed => (
            FORMWORK_CONTRACT_ID,
            FORMWORK_CONTRACT_BYTES,
            FORMWORK_CONTRACT_SHA256,
        ),
    }
}

fn calculate_layout_checksum(
    size: PerfScenarioSize,
    phase: PerfWallPhase,
    specimens: &[SpecimenSpec],
) -> String {
    let mut digest = Sha256::new();
    digest.update(match phase {
        PerfWallPhase::Completed | PerfWallPhase::Provisional => {
            b"wall-density-v1-layout-v1\0".as_slice()
        }
        PerfWallPhase::Mixed => b"wall-formwork-density-v1-layout-v1\0".as_slice(),
    });
    digest.update(target_size_name(size).as_bytes());
    digest.update([0]);
    digest.update(phase.as_str().as_bytes());
    digest.update([0]);
    for specimen in specimens {
        digest.update(specimen.ordinal.to_le_bytes());
        digest.update(specimen.grid.0.to_le_bytes());
        digest.update(specimen.grid.1.to_le_bytes());
        digest.update([specimen.mask]);
        for (connector, _) in &specimen.connectors {
            digest.update(connector.direction.as_bytes());
            digest.update([0]);
            digest.update(connector.grid.0.to_le_bytes());
            digest.update(connector.grid.1.to_le_bytes());
        }
        digest.update([0xff]);
    }
    digest_hex(&digest.finalize().into())
}

fn contract_sha256(bytes: &[u8]) -> String {
    digest_hex(&Sha256::digest(bytes).into())
}

fn fail_fixture(
    state: &mut WallDensityFixtureState,
    exit: &mut MessageWriter<AppExit>,
    reason: String,
) {
    error!("WALL_DENSITY_FIXTURE: {reason}");
    state.phase = WallDensityFixturePhase::Failed;
    state.failure = Some(reason);
    exit.write(AppExit::error());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_have_exact_mask_and_connector_counts() {
        for (size, expected_targets, expected_connectors, repetitions) in [
            (PerfScenarioSize::Small, 96, 192, 6),
            (PerfScenarioSize::Medium, 384, 768, 24),
        ] {
            let layout = WallDensityLayout::build(size, PerfWallPhase::Completed);
            assert_eq!(layout.specimens.len(), expected_targets);
            assert_eq!(layout.connector_count(), expected_connectors);
            assert_eq!(layout.mask_counts().len(), MASK_COUNT);
            assert!(
                layout
                    .mask_counts()
                    .values()
                    .all(|&count| count == repetitions)
            );
            let mut cells = HashSet::new();
            for specimen in &layout.specimens {
                assert!(cells.insert(specimen.grid));
                for (connector, _) in &specimen.connectors {
                    assert!(cells.insert(connector.grid));
                }
            }
        }
    }

    #[test]
    fn layout_checksums_are_phase_and_size_specific() {
        let checksums = [
            WallDensityLayout::build(PerfScenarioSize::Small, PerfWallPhase::Completed)
                .layout_checksum,
            WallDensityLayout::build(PerfScenarioSize::Small, PerfWallPhase::Provisional)
                .layout_checksum,
            WallDensityLayout::build(PerfScenarioSize::Medium, PerfWallPhase::Completed)
                .layout_checksum,
            WallDensityLayout::build(PerfScenarioSize::Medium, PerfWallPhase::Provisional)
                .layout_checksum,
            WallDensityLayout::build(PerfScenarioSize::Medium, PerfWallPhase::Mixed)
                .layout_checksum,
        ];
        assert_eq!(
            checksums,
            [
                "2d6ebb05e0ddea502fa49ac832991f4dcd7e1c6109149cebd7c62d61bedd64bf",
                "436cf15c38d26909d22dc36bfcc1118bcc04ba0aeb499b6308080e80172e6788",
                "bea0ceea470cb3e409bd17113b4b89923c81c9114f967f9019f765c9d4b7b642",
                "9587bebc53d564b3e0d6ccbdea176c6c0c2b4165ffcafbd4caed82887a399c90",
                "32e559d4ec6c7bd2e6a641d0f32c1b7fa3936b15d7e9c095fecdb589d88efe4a",
            ]
        );
    }

    #[test]
    fn embedded_contract_hash_is_frozen() {
        assert_eq!(contract_sha256(CONTRACT_BYTES), CONTRACT_SHA256);
        assert_eq!(
            contract_sha256(FORMWORK_CONTRACT_BYTES),
            FORMWORK_CONTRACT_SHA256
        );
    }

    #[test]
    fn mixed_layout_balances_each_mask_across_both_phases() {
        let layout = WallDensityLayout::build(PerfScenarioSize::Medium, PerfWallPhase::Mixed);
        let provisional = layout
            .specimens
            .iter()
            .filter(|specimen| layout.is_provisional(specimen.ordinal))
            .count();
        assert_eq!(provisional, 192);
        assert_eq!(layout.specimens.len() - provisional, 192);
        for mask in 0..MASK_COUNT {
            let mask = u8::try_from(mask).unwrap();
            let specimens = layout
                .specimens
                .iter()
                .filter(|specimen| specimen.mask == mask);
            let provisional = specimens
                .clone()
                .filter(|specimen| layout.is_provisional(specimen.ordinal))
                .count();
            assert_eq!(provisional, 12);
            assert_eq!(specimens.count() - provisional, 12);
        }
        assert_eq!(
            layout.phase_mask_counts(false),
            layout.phase_mask_counts(true)
        );
    }

    #[test]
    fn mixed_gallery_evidence_keeps_density_targets_and_adds_two_depth_subjects() {
        let mut layout = WallDensityLayout::build(PerfScenarioSize::Medium, PerfWallPhase::Mixed);
        for specimen in &mut layout.specimens {
            specimen.wall = Some(Entity::from_bits(u64::from(specimen.ordinal) + 1));
        }
        let pair = MixedGalleryPair {
            provisional: Entity::from_bits(1_001),
            completed: Entity::from_bits(1_002),
        };
        let state = WallDensityFixtureState {
            phase: WallDensityFixturePhase::Ready,
            layout: Some(layout),
            mixed_gallery_pair: Some(pair),
            failure: None,
        };

        assert_eq!(
            state.soul_depth_subjects(),
            Some([
                (Entity::from_bits(65), (22, 17), true),
                (Entity::from_bits(85), (22, 22), false),
            ])
        );
        let evidence = state.renderdoc_evidence().unwrap();
        assert_eq!(evidence.target_wall_count, 384);
        assert_eq!(evidence.target_entities.len(), 386);
        assert_eq!(evidence.completed_wall_count, 192);
        assert_eq!(evidence.provisional_wall_count, 192);
        assert_eq!(evidence.completed_mask_counts.len(), 16);
        assert_eq!(evidence.provisional_mask_counts.len(), 16);
        assert_eq!(
            evidence.mixed_gallery_pair,
            Some([
                (pair.provisional, (20, 15), true),
                (pair.completed, (21, 15), false),
            ])
        );
    }

    #[test]
    fn door_connector_enters_the_production_topology_mirror() {
        let grid = (12, 34);
        let (blueprint, mirror) = door_connector_blueprint(grid);
        assert_eq!(blueprint.kind, BuildingType::Door);
        assert_eq!(blueprint.occupied_grids, vec![grid]);
        assert!(mirror.is_wall_or_door);
        assert!(!mirror.is_plain_wall);
        assert_eq!(mirror.occupied_grids, vec![grid]);
    }

    #[test]
    fn ready_state_writes_exact_summary_and_layout_rows() {
        let layout = WallDensityLayout::build(PerfScenarioSize::Small, PerfWallPhase::Provisional);
        let state = WallDensityFixtureState {
            phase: WallDensityFixturePhase::Ready,
            layout: Some(layout),
            mixed_gallery_pair: None,
            failure: None,
        };

        let (summary, layout_csv) = state.sidecars().unwrap();

        assert_eq!(summary["contract_id"], CONTRACT_ID);
        assert_eq!(summary["contract_sha256"], CONTRACT_SHA256);
        assert_eq!(summary["target_size"], "N");
        assert_eq!(summary["phase"], "provisional");
        assert_eq!(summary["target_wall_count"], 96);
        assert_eq!(summary["connector_count"], 192);
        assert_eq!(summary["mask_counts"]["0000"], 6);
        assert_eq!(summary["mask_counts"]["1111"], 6);
        assert_eq!(layout_csv.lines().count(), 1 + 96 + 192);
    }
}
