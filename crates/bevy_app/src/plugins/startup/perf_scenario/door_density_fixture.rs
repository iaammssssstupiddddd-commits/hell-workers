//! Frozen Door-density fixture and presentation evidence for production acceptance.

use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
use hw_jobs::{Building, BuildingType, Door, DoorState};
use hw_visual::TopDownStructuralMaterial;
use hw_visual::visual3d::{
    Building3dVisual, Door3dPresentationMode, DoorPresentationAxis, DoorPresentationState,
};
use hw_world::{WorldMap, WorldMapRead, WorldMapWrite};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::config::PerfDoorPresentation;
use super::fixture::{
    PerfFixtureKind, PerfFixtureMarker, PerfMainCameraQuery, PerfScenarioApplied,
};
use super::{PerfScenarioConfig, PerfScenarioSize, PerfWorkload};
use crate::assets::door_asset_set::{
    DoorAssetFallbackReason, DoorAssetReadiness, DoorAssetReadinessState, ProductionDoorAssetPool,
    ProductionDoorMaterialPool,
};
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::spawn_building_3d_visual;
use crate::systems::jobs::wall_construction::spawn_wall_shell;

pub(super) const CONTRACT_ID: &str = "door-density-v1";
const CONTRACT_BYTES: &[u8] = b"door-density-v1\nseed=20260906\norigin=12,12\nstride=5,5\ncolumns=8\nsmall=32\nmedium=128\naxis=even-ew,odd-ns\nstate=closed,open,locked\nsupport_walls=2\ncamera_scale=5\n";
const GRID_ORIGIN: (i32, i32) = (12, 12);
const GRID_STRIDE: (i32, i32) = (5, 5);
const GRID_COLUMNS: usize = 8;
pub(super) const CAMERA_SCALE: f32 = 5.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DoorDensityFixturePhase {
    #[default]
    Inactive,
    Spawned,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
struct DoorSpecimen {
    ordinal: u32,
    grid: (i32, i32),
    axis: DoorPresentationAxis,
    state: DoorState,
    door: Option<Entity>,
    supports: [(i32, i32); 2],
    support_entities: [Option<Entity>; 2],
}

#[derive(Clone, Debug)]
struct DoorDensityLayout {
    size: PerfScenarioSize,
    specimens: Vec<DoorSpecimen>,
    layout_checksum: String,
}

impl DoorDensityLayout {
    fn build(size: PerfScenarioSize) -> Self {
        let mut specimens = Vec::with_capacity(target_count(size));
        for ordinal in 0..target_count(size) {
            let column = ordinal % GRID_COLUMNS;
            let row = ordinal / GRID_COLUMNS;
            let grid = (
                GRID_ORIGIN.0 + i32::try_from(column).expect("column fits i32") * GRID_STRIDE.0,
                GRID_ORIGIN.1 + i32::try_from(row).expect("row fits i32") * GRID_STRIDE.1,
            );
            let axis = if ordinal % 2 == 0 {
                DoorPresentationAxis::EastWest
            } else {
                DoorPresentationAxis::NorthSouth
            };
            let state = match ordinal % 3 {
                0 => DoorState::Closed,
                1 => DoorState::Open,
                _ => DoorState::Locked,
            };
            let supports = match axis {
                DoorPresentationAxis::EastWest => [(grid.0 - 1, grid.1), (grid.0 + 1, grid.1)],
                DoorPresentationAxis::NorthSouth => [(grid.0, grid.1 - 1), (grid.0, grid.1 + 1)],
            };
            specimens.push(DoorSpecimen {
                ordinal: u32::try_from(ordinal).expect("ordinal fits u32"),
                grid,
                axis,
                state,
                door: None,
                supports,
                support_entities: [None, None],
            });
        }
        let layout_checksum = calculate_layout_checksum(size, &specimens);
        Self {
            size,
            specimens,
            layout_checksum,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DoorDensityPresentationEvidence {
    expected_mode: &'static str,
    target_door_count: usize,
    support_wall_count: usize,
    visual_count: usize,
    production_count: usize,
    fallback_count: usize,
    active_mesh_count: usize,
    active_material_count: usize,
    resident_production_mesh_count: usize,
    resident_production_material_count: usize,
    resident_fallback_mesh_count: usize,
    resident_fallback_material_count: usize,
    resident_production_image_count: usize,
    state_counts: BTreeMap<&'static str, usize>,
    axis_counts: BTreeMap<&'static str, usize>,
    session_id: u64,
    readiness_revision: u64,
    asset_set_generation: u64,
    authority: crate::assets::door_asset_set::DoorAssetAuthority,
    manifest_sha256: String,
}

#[derive(Resource, Default)]
pub(crate) struct DoorDensityFixtureState {
    phase: DoorDensityFixturePhase,
    layout: Option<DoorDensityLayout>,
    presentation: Option<DoorDensityPresentationEvidence>,
    failure: Option<String>,
}

impl DoorDensityFixtureState {
    pub(super) fn sidecars(&self) -> Result<(serde_json::Value, String), String> {
        if self.phase != DoorDensityFixturePhase::Ready {
            return Err(format!(
                "door-density sidecar requested in {:?} phase",
                self.phase
            ));
        }
        let layout = self
            .layout
            .as_ref()
            .ok_or_else(|| "door-density Ready state has no layout".to_string())?;
        let presentation = self
            .presentation
            .as_ref()
            .ok_or_else(|| "door-density Ready state has no presentation evidence".to_string())?;
        let summary = serde_json::json!({
            "schema_version": 1,
            "contract_id": CONTRACT_ID,
            "contract_sha256": format!("{:x}", Sha256::digest(CONTRACT_BYTES)),
            "layout_checksum": layout.layout_checksum,
            "target_size": target_size_name(layout.size),
            "perf_size": layout.size.as_str(),
            "target_door_count": layout.specimens.len(),
            "support_wall_count": layout.specimens.len() * 2,
            "grid": {
                "origin": [GRID_ORIGIN.0, GRID_ORIGIN.1],
                "stride": [GRID_STRIDE.0, GRID_STRIDE.1],
                "columns": GRID_COLUMNS,
            },
            "camera_scale": CAMERA_SCALE,
            "stable": true,
            "initial": presentation,
            "final": presentation,
        });
        let mut csv = String::from(
            "schema_version,record_kind,ordinal,target_ordinal,grid_x,grid_y,axis,state\n",
        );
        let mut support_ordinal = 0usize;
        for specimen in &layout.specimens {
            csv.push_str(&format!(
                "1,target,{},{},{},{},{},{}\n",
                specimen.ordinal,
                specimen.ordinal,
                specimen.grid.0,
                specimen.grid.1,
                axis_name(specimen.axis),
                state_name(specimen.state),
            ));
            for support in specimen.supports {
                csv.push_str(&format!(
                    "1,support,{support_ordinal},{},{},{},{},{}\n",
                    specimen.ordinal,
                    support.0,
                    support.1,
                    axis_name(specimen.axis),
                    state_name(specimen.state),
                ));
                support_ordinal += 1;
            }
        }
        Ok((summary, csv))
    }
}

pub(super) struct DoorDensitySetupContext<'a, 'w, 's> {
    pub(super) commands: &'a mut Commands<'w, 's>,
    pub(super) state: &'a mut DoorDensityFixtureState,
    pub(super) world_map: &'a mut WorldMapWrite<'w>,
    pub(super) handles_3d: &'a Building3dHandles,
    pub(super) q_main_camera: &'a mut PerfMainCameraQuery<'w, 's>,
    pub(super) exit: &'a mut MessageWriter<'w, AppExit>,
}

pub(super) fn begin_door_density_fixture(
    config: &PerfScenarioConfig,
    context: DoorDensitySetupContext<'_, '_, '_>,
) {
    let DoorDensitySetupContext {
        commands,
        state,
        world_map,
        handles_3d,
        q_main_camera,
        exit,
    } = context;
    if state.phase != DoorDensityFixturePhase::Inactive {
        return;
    }
    let mut layout = DoorDensityLayout::build(config.size);
    if let Err(reason) = validate_layout_cells(&layout, world_map.as_ref()) {
        fail_fixture(state, exit, reason);
        return;
    }
    let Ok(mut camera) = q_main_camera.single_mut() else {
        fail_fixture(
            state,
            exit,
            "door-density requires exactly one main Camera2d".to_string(),
        );
        return;
    };
    let first = WorldMap::grid_to_world(GRID_ORIGIN.0, GRID_ORIGIN.1);
    let last = WorldMap::grid_to_world(
        GRID_ORIGIN.0 + 7 * GRID_STRIDE.0,
        GRID_ORIGIN.1
            + i32::try_from((target_count(config.size) - 1) / GRID_COLUMNS).expect("row fits i32")
                * GRID_STRIDE.1,
    );
    camera.translation.x = (first.x + last.x) * 0.5;
    camera.translation.y = (first.y + last.y) * 0.5;
    camera.scale = Vec3::new(CAMERA_SCALE, CAMERA_SCALE, 1.0);

    for specimen in &mut layout.specimens {
        for (index, grid) in specimen.supports.into_iter().enumerate() {
            let wall = spawn_wall_shell(commands, handles_3d, grid, false);
            commands.entity(wall).insert((
                BuildingVisualState {
                    kind: BuildingTypeVisual::Wall,
                    is_provisional: false,
                },
                PerfFixtureMarker {
                    kind: PerfFixtureKind::DoorDensitySupport,
                    ordinal: specimen.ordinal,
                },
            ));
            world_map.reserve_building_footprint(BuildingType::Wall, wall, std::iter::once(grid));
            specimen.support_entities[index] = Some(wall);
        }
        let position = WorldMap::grid_to_world(specimen.grid.0, specimen.grid.1);
        let door = commands
            .spawn((
                Building {
                    kind: BuildingType::Door,
                    is_provisional: false,
                },
                BuildingVisualState {
                    kind: BuildingTypeVisual::Door,
                    is_provisional: false,
                },
                Door {
                    state: specimen.state,
                },
                Transform::from_xyz(position.x, position.y, 0.0),
                Visibility::Inherited,
                PerfFixtureMarker {
                    kind: PerfFixtureKind::DoorDensityTarget,
                    ordinal: specimen.ordinal,
                },
                Name::new(format!("PerfDoorDensity ({})", specimen.ordinal)),
            ))
            .id();
        spawn_building_3d_visual(
            commands,
            door,
            BuildingType::Door,
            position,
            false,
            handles_3d,
        );
        world_map.reserve_building_footprint(
            BuildingType::Door,
            door,
            std::iter::once(specimen.grid),
        );
        world_map.register_door(specimen.grid, door, specimen.state);
        specimen.door = Some(door);
    }
    state.layout = Some(layout);
    state.phase = DoorDensityFixturePhase::Spawned;
}

type DoorVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static DoorPresentationState,
        &'static DoorPresentationAxis,
        &'static Door3dPresentationMode,
        &'static Mesh3d,
        &'static MeshMaterial3d<TopDownStructuralMaterial>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct DoorDensityValidationParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    state: ResMut<'w, DoorDensityFixtureState>,
    applied: ResMut<'w, PerfScenarioApplied>,
    world_map: WorldMapRead<'w>,
    buildings: Query<'w, 's, (&'static Building, &'static Transform, Option<&'static Door>)>,
    visuals: DoorVisualQuery<'w, 's>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: Res<'w, Assets<TopDownStructuralMaterial>>,
    images: Res<'w, Assets<Image>>,
    fallback: Res<'w, Building3dHandles>,
    production: Res<'w, ProductionDoorAssetPool>,
    production_material: Res<'w, ProductionDoorMaterialPool>,
    readiness: Res<'w, DoorAssetReadiness>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn validate_door_density_fixture_system(mut params: DoorDensityValidationParams) {
    if !params.config.enabled()
        || params.config.workload != PerfWorkload::DoorDensity
        || matches!(
            params.state.phase,
            DoorDensityFixturePhase::Inactive | DoorDensityFixturePhase::Failed
        )
    {
        return;
    }
    match inspect_fixture(&params) {
        Ok(None) => {}
        Ok(Some(evidence)) => {
            if let Some(initial) = params.state.presentation.as_ref() {
                if initial != &evidence {
                    fail_fixture(
                        &mut params.state,
                        &mut params.exit,
                        "door-density presentation changed after becoming ready".to_string(),
                    );
                }
            } else {
                params.state.presentation = Some(evidence);
                params.state.phase = DoorDensityFixturePhase::Ready;
                params.applied.workload = true;
            }
        }
        Err(reason) => fail_fixture(&mut params.state, &mut params.exit, reason),
    }
}

fn inspect_fixture(
    params: &DoorDensityValidationParams<'_, '_>,
) -> Result<Option<DoorDensityPresentationEvidence>, String> {
    let layout = params
        .state
        .layout
        .as_ref()
        .ok_or_else(|| "door-density active state has no layout".to_string())?;
    let expected = params
        .config
        .door_presentation()
        .ok_or_else(|| "door-density presentation is absent".to_string())?;
    let resolved = params
        .production
        .resolved
        .as_ref()
        .ok_or_else(|| "door-density production asset pool is unresolved".to_string());
    let resolved = match resolved {
        Ok(value) => value,
        Err(_) if matches!(params.readiness.state, DoorAssetReadinessState::Loading) => {
            return Ok(None);
        }
        Err(reason) => return Err(reason),
    };
    match (&params.readiness.state, expected) {
        (DoorAssetReadinessState::Loading, _) => return Ok(None),
        (DoorAssetReadinessState::Eligible(identity), PerfDoorPresentation::Production)
            if identity == &resolved.identity => {}
        (
            DoorAssetReadinessState::Fallback(DoorAssetFallbackReason::CandidateDisabled),
            PerfDoorPresentation::FallbackControl,
        ) => {}
        (state, _) => {
            return Err(format!(
                "door-density readiness cannot satisfy {}: {state:?}",
                expected.as_str()
            ));
        }
    }
    if params.production_material.identity.as_ref() != Some(&resolved.identity)
        || params.production_material.material.is_none()
    {
        return Ok(None);
    }

    let mut target_entities = HashSet::new();
    for specimen in &layout.specimens {
        let door = specimen
            .door
            .ok_or_else(|| format!("door-density target {} has no entity", specimen.ordinal))?;
        target_entities.insert(door);
        let (building, transform, semantic) = params
            .buildings
            .get(door)
            .map_err(|_| format!("door-density target {} is missing", specimen.ordinal))?;
        if building.kind != BuildingType::Door
            || building.is_provisional
            || semantic.map(|door| door.state) != Some(specimen.state)
            || WorldMap::world_to_grid(transform.translation.truncate()) != specimen.grid
            || params.world_map.building_entity(specimen.grid) != Some(door)
            || params
                .world_map
                .door_state(specimen.grid.0, specimen.grid.1)
                != Some(specimen.state)
        {
            return Err(format!("door-density target {} differs", specimen.ordinal));
        }
        for (grid, entity) in specimen.supports.into_iter().zip(specimen.support_entities) {
            let entity = entity.ok_or_else(|| {
                format!("door-density target {} has no support", specimen.ordinal)
            })?;
            let (support, transform, door) = params
                .buildings
                .get(entity)
                .map_err(|_| format!("door-density support {:?} is missing", grid))?;
            if support.kind != BuildingType::Wall
                || support.is_provisional
                || door.is_some()
                || WorldMap::world_to_grid(transform.translation.truncate()) != grid
                || params.world_map.building_entity(grid) != Some(entity)
            {
                return Err(format!("door-density support {:?} differs", grid));
            }
        }
    }

    let mut owners = HashMap::<Entity, usize>::new();
    let mut production_count = 0usize;
    let mut fallback_count = 0usize;
    let mut active_meshes = HashSet::new();
    let mut active_materials = HashSet::new();
    let mut state_counts = BTreeMap::new();
    let mut axis_counts = BTreeMap::new();
    for (visual, state, axis, mode, mesh, material) in &params.visuals {
        if !target_entities.contains(&visual.owner) {
            continue;
        }
        *owners.entry(visual.owner).or_default() += 1;
        match mode {
            Door3dPresentationMode::Production => production_count += 1,
            Door3dPresentationMode::Fallback => fallback_count += 1,
        }
        *state_counts
            .entry(presentation_state_name(*state))
            .or_default() += 1;
        *axis_counts.entry(axis_name(*axis)).or_default() += 1;
        active_meshes.insert(mesh.0.id());
        active_materials.insert(material.0.id());
    }
    if owners.len() != layout.specimens.len() || owners.values().any(|count| *count != 1) {
        return Err("door-density visual owner coverage differs".to_string());
    }
    let expected_count = layout.specimens.len();
    match expected {
        PerfDoorPresentation::Production if fallback_count == expected_count => return Ok(None),
        PerfDoorPresentation::Production
            if production_count != expected_count || fallback_count != 0 =>
        {
            return Err("door-density production presentation is mixed".to_string());
        }
        PerfDoorPresentation::FallbackControl
            if fallback_count != expected_count || production_count != 0 =>
        {
            return Err("door-density fallback presentation is mixed".to_string());
        }
        _ => {}
    }
    let expected_state_counts = expected_state_counts(expected_count);
    if state_counts != expected_state_counts
        || axis_counts
            != BTreeMap::from([
                ("east_west", expected_count.div_ceil(2)),
                ("north_south", expected_count / 2),
            ])
    {
        return Err("door-density state or axis distribution differs".to_string());
    }
    let resident_production_mesh_count = resolved
        .meshes
        .iter()
        .filter(|handle| params.meshes.contains(handle.id()))
        .count();
    let resident_production_material_count = usize::from(
        params
            .production_material
            .material
            .as_ref()
            .is_some_and(|handle| params.materials.contains(handle.id())),
    );
    let resident_fallback_mesh_count =
        usize::from(params.meshes.contains(params.fallback.door_mesh.id()));
    let resident_fallback_material_count = [
        &params.fallback.door_closed_material,
        &params.fallback.door_open_material,
        &params.fallback.door_locked_material,
    ]
    .into_iter()
    .filter(|handle| params.materials.contains(handle.id()))
    .count();
    let resident_production_image_count =
        [&resolved.albedo, &resolved.preview_ew, &resolved.preview_ns]
            .into_iter()
            .filter(|handle| params.images.contains(handle.id()))
            .count();
    if resident_production_mesh_count != 3
        || resident_production_material_count != 1
        || resident_fallback_mesh_count != 1
        || resident_fallback_material_count != 3
        || resident_production_image_count != 3
    {
        return Err("door-density finite asset pool differs".to_string());
    }
    Ok(Some(DoorDensityPresentationEvidence {
        expected_mode: expected.as_str(),
        target_door_count: expected_count,
        support_wall_count: expected_count * 2,
        visual_count: owners.len(),
        production_count,
        fallback_count,
        active_mesh_count: active_meshes.len(),
        active_material_count: active_materials.len(),
        resident_production_mesh_count,
        resident_production_material_count,
        resident_fallback_mesh_count,
        resident_fallback_material_count,
        resident_production_image_count,
        state_counts,
        axis_counts,
        session_id: params.readiness.session_id,
        readiness_revision: params.readiness.activation_revision,
        asset_set_generation: resolved.identity.asset_set_generation,
        authority: resolved.identity.authority,
        manifest_sha256: resolved.identity.manifest_sha256.clone(),
    }))
}

fn validate_layout_cells(layout: &DoorDensityLayout, world_map: &WorldMap) -> Result<(), String> {
    let mut cells = HashSet::new();
    for specimen in &layout.specimens {
        for grid in std::iter::once(specimen.grid).chain(specimen.supports) {
            if grid.0 < 0 || grid.0 >= MAP_WIDTH || grid.1 < 0 || grid.1 >= MAP_HEIGHT {
                return Err(format!("door-density cell {grid:?} is outside the map"));
            }
            if !cells.insert(grid) || world_map.has_building(grid) {
                return Err(format!("door-density cell {grid:?} is unavailable"));
            }
        }
    }
    Ok(())
}

fn target_count(size: PerfScenarioSize) -> usize {
    match size {
        PerfScenarioSize::Small => 32,
        PerfScenarioSize::Medium => 128,
        PerfScenarioSize::Large => unreachable!("door-density rejects large"),
    }
}

fn target_size_name(size: PerfScenarioSize) -> &'static str {
    match size {
        PerfScenarioSize::Small => "N",
        PerfScenarioSize::Medium => "4N",
        PerfScenarioSize::Large => unreachable!("door-density rejects large"),
    }
}

fn state_name(state: DoorState) -> &'static str {
    match state {
        DoorState::Closed => "closed",
        DoorState::Open => "open",
        DoorState::Locked => "locked",
    }
}

fn presentation_state_name(state: DoorPresentationState) -> &'static str {
    match state {
        DoorPresentationState::Closed => "closed",
        DoorPresentationState::Open => "open",
        DoorPresentationState::Locked => "locked",
    }
}

fn axis_name(axis: DoorPresentationAxis) -> &'static str {
    match axis {
        DoorPresentationAxis::EastWest => "east_west",
        DoorPresentationAxis::NorthSouth => "north_south",
    }
}

fn expected_state_counts(count: usize) -> BTreeMap<&'static str, usize> {
    BTreeMap::from([
        ("closed", count.div_ceil(3)),
        ("open", (count + 1) / 3),
        ("locked", count / 3),
    ])
}

fn calculate_layout_checksum(size: PerfScenarioSize, specimens: &[DoorSpecimen]) -> String {
    let mut digest = Sha256::new();
    digest.update(CONTRACT_BYTES);
    digest.update(target_size_name(size).as_bytes());
    for specimen in specimens {
        digest.update(specimen.ordinal.to_le_bytes());
        digest.update(specimen.grid.0.to_le_bytes());
        digest.update(specimen.grid.1.to_le_bytes());
        digest.update(axis_name(specimen.axis).as_bytes());
        digest.update(state_name(specimen.state).as_bytes());
        for support in specimen.supports {
            digest.update(support.0.to_le_bytes());
            digest.update(support.1.to_le_bytes());
        }
    }
    format!("{:x}", digest.finalize())
}

fn fail_fixture(
    state: &mut DoorDensityFixtureState,
    exit: &mut MessageWriter<AppExit>,
    reason: String,
) {
    error!("DOOR_DENSITY_FIXTURE: {reason}");
    state.phase = DoorDensityFixturePhase::Failed;
    state.failure = Some(reason);
    exit.write(AppExit::error());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_have_frozen_counts_and_distributions() {
        for (size, targets, walls, states) in [
            (PerfScenarioSize::Small, 32, 64, [11, 11, 10]),
            (PerfScenarioSize::Medium, 128, 256, [43, 43, 42]),
        ] {
            let layout = DoorDensityLayout::build(size);
            assert_eq!(layout.specimens.len(), targets);
            assert_eq!(layout.specimens.len() * 2, walls);
            assert_eq!(
                expected_state_counts(targets),
                BTreeMap::from([
                    ("closed", states[0]),
                    ("open", states[1]),
                    ("locked", states[2]),
                ])
            );
            assert_eq!(
                layout
                    .specimens
                    .iter()
                    .filter(|specimen| specimen.axis == DoorPresentationAxis::EastWest)
                    .count(),
                targets / 2
            );
        }
    }

    #[test]
    fn layout_cells_are_unique_and_in_bounds() {
        for size in [PerfScenarioSize::Small, PerfScenarioSize::Medium] {
            let layout = DoorDensityLayout::build(size);
            let cells = layout
                .specimens
                .iter()
                .flat_map(|specimen| std::iter::once(specimen.grid).chain(specimen.supports))
                .collect::<HashSet<_>>();
            assert_eq!(cells.len(), layout.specimens.len() * 3);
            assert!(
                cells
                    .iter()
                    .all(|(x, y)| { *x >= 0 && *x < MAP_WIDTH && *y >= 0 && *y < MAP_HEIGHT })
            );
        }
    }
}
