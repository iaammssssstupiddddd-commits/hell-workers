use std::collections::{BTreeMap, BTreeSet, HashSet};

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::{StoredIn, TaskWorkers, WorkingOn};
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerAllocationMode, PowerConsumer,
    PowerGenerator, PowerGrid, PowerGridAllocationSummary, PowerShedReason, PowerSupplyState,
    SOUL_SPA_MAX_ACTIVE_SLOTS, SoulSpaPhase, SoulSpaSite, SoulSpaTile, Unpowered, YardPowerGrid,
};
use hw_jobs::{
    ActiveTaskIdentity, BonePile, BridgeMarker, GeneratePowerData, GeneratePowerPhase, RestArea,
    SandPile,
};
use hw_visual::visual3d::Building3dVisual;
use hw_world::{DoorVisualHandles, Room, RoomBoundaryLookup, RoomTileLookup, Yard};

use super::fixture::{
    PerfSetupFamiliarFilter, PerfSetupFamiliarQuery, PerfSetupSoulFilter, PerfSetupSoulQuery,
};
use super::{
    ActiveCommand, AssignedTask, BuildingType, DamnedSoul, Destination, FamiliarCommand,
    FamiliarOperation, FamiliarPolicy, Path, PerfAuditActorRecord, PerfScenarioApplied,
    PerfScenarioConfig, PerfScenarioSize, PerfWorkload, WorldMap, WorldMapWrite,
    write_building_type, write_door_state, write_f32, write_grid_pos, write_transform, write_u64,
    write_vec2, write_work_type,
};
use crate::assets::GameAssets;
use crate::interface::selection::building_place::try_place_bucket_storage_companion;
use crate::interface::selection::soul_spa_place::spawn_soul_spa;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::floor_construction::{
    register_completed_floors, spawn_completed_floor_tile,
};
use crate::systems::jobs::wall_construction::spawn_wall_shell;
use crate::systems::jobs::{
    Blueprint, Building, Designation, Door, MudMixerStorage, TaskSlots, WorkType,
};
use crate::systems::logistics::{
    BelongsTo, BucketStorage, PendingBelongsToBlueprint, ResourceItem, ResourceType, Stockpile,
    Wheelbarrow, WheelbarrowParking,
};
use crate::world::map::RIVER_Y_MIN;

pub(super) const CONTRACT_ID: &str = "rtt-light-v1";
#[cfg(test)]
pub(super) const STAGE_ID: &str = "current";
#[cfg(test)]
pub(super) const LANE: &str = "static";
pub(super) const FIXTURE_ID: &str = "indoor-light-grid-v1";
pub(super) const CONTRACT_SHA256: &str =
    "121a365ac3349cd4fa7890ab3069f0392098ced17e0d47f920095a1490c2ba11";
pub(super) const FIXTURE_SHA256: &str =
    "a82e4d8b0a9d3962877f8b43f047ed424189ae257a21428ebb50cac851a9b1df";
pub(super) const SMALL_LAYOUT_SHA256: &str =
    "e87a3b1aeb7ee1fbe334d311ad731bef24ce90ec80066af1e35c006ef4273af2";
pub(super) const MEDIUM_LAYOUT_SHA256: &str =
    "e18320b3bcf8089c1ea2743003eadd79a0c938caa44682ed414e9d9d54af8f2d";
pub(super) const LARGE_LAYOUT_SHA256: &str =
    "3dec65d6c30ee9b88678af28a818a05fa70ededc66f20242ff78dcb6772c56fd";

const ORIGIN: (i32, i32) = (16, 20);
const MODULE_EXTENT: i32 = 7;
const ROOM_INTERIOR_TILES: usize = 36;
type Grid = (i32, i32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum IndoorLightFixturePhase {
    #[default]
    Inactive,
    Settling,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
pub(super) struct IndoorLightFixtureObservation {
    pub(super) case_id: String,
    pub(super) layout_checksum: &'static str,
    pub(super) floors: usize,
    pub(super) walls: usize,
    pub(super) doors: usize,
    pub(super) rooms: usize,
    pub(super) room_tiles: usize,
    pub(super) room_boundaries: usize,
    pub(super) souls: usize,
    pub(super) familiars: usize,
    pub(super) soul_spas: usize,
    pub(super) generator_souls: usize,
    pub(super) yards: usize,
    pub(super) main_generation: f32,
    pub(super) main_demand: f32,
    pub(super) main_headroom: f32,
    pub(super) main_supplied_count: usize,
    pub(super) main_shed_count: usize,
    pub(super) control_generation: f32,
    pub(super) control_demand: f32,
    pub(super) control_supplied_count: usize,
    pub(super) control_shed_count: usize,
    presentation: Vec<IndoorLightPresentationObservation>,
}

#[derive(Clone, Debug)]
struct IndoorLightPresentationObservation {
    building_kind: &'static str,
    entity_count: usize,
    root_sprite_count: usize,
    child_sprite_count: usize,
    owner_3d_count: usize,
}

#[derive(Resource, Default)]
pub(crate) struct IndoorLightFixtureState {
    pub(super) phase: IndoorLightFixturePhase,
    fixture: Option<IndoorLightFixtureEntities>,
    audit_entities: Option<IndoorLightAuditEntities>,
    pub(super) observation: Option<IndoorLightFixtureObservation>,
    pub(super) failure: Option<String>,
    door_states_seeded: bool,
}

#[derive(Clone)]
struct IndoorLightAuditBuilding {
    entity: Entity,
    kind: BuildingType,
    ordinal: u32,
    anchor: Grid,
    draw_pos: Vec2,
    door_state: Option<hw_core::world::DoorState>,
    lamp_role: Option<LampRole>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LampRole {
    Main,
    Control,
}

#[derive(Clone)]
struct IndoorLightAuditSpa {
    site: Entity,
    tiles: Vec<Entity>,
}

#[derive(Clone)]
struct IndoorLightAuditEntities {
    buildings: Vec<IndoorLightAuditBuilding>,
    yards: [Entity; 2],
    grids: [Entity; 2],
    spas: Vec<IndoorLightAuditSpa>,
}

impl IndoorLightFixtureState {
    pub(super) fn behavior_subjects(&self) -> Option<(Entity, Entity, Grid)> {
        let fixture = self.fixture.as_ref()?;
        let audit = self.audit_entities.as_ref()?;
        let door = audit
            .buildings
            .iter()
            .find(|building| building.kind == BuildingType::Door && building.ordinal == 0)?;
        Some((*fixture.soul_entities.first()?, door.entity, door.anchor))
    }

    pub(super) fn behavior_fixture_checksum(&self) -> Option<&'static str> {
        self.fixture
            .as_ref()
            .map(|fixture| fixture.layout.layout_checksum)
    }

    pub(super) fn sidecar_csvs(
        &self,
        stage_id: &str,
        lane: &str,
    ) -> std::io::Result<(String, String, String)> {
        if self.phase != IndoorLightFixturePhase::Ready {
            return Err(std::io::Error::other(format!(
                "indoor-light fixture sidecar requested in {:?} phase",
                self.phase
            )));
        }
        let observation = self.observation.as_ref().ok_or_else(|| {
            std::io::Error::other("indoor-light fixture is Ready without an observation")
        })?;
        let fixture = self.fixture.as_ref().ok_or_else(|| {
            std::io::Error::other("indoor-light fixture is Ready without fixture entities")
        })?;

        let summary = format!(
            concat!(
                "schema_version,contract_id,stage_id,lane,checkpoint,case_id,fixture_id,size,",
                "layout_checksum,measurement_contract_sha256,fixture_contract_sha256,",
                "completed_floors,completed_walls,doors,supplied_lamp_candidates,",
                "unsupplied_lamp_candidates,rooms,room_tiles,room_boundary_lookup_cells,",
                "souls,familiars,yards,operational_soul_spas,generator_souls,main_generation,",
                "main_demand,main_headroom,main_supplied_count,main_shed_count,",
                "control_generation,control_demand,control_supplied_count,control_shed_count\n",
                "1,{contract_id},{stage_id},{lane},fixture-pre-update,{},",
                "{fixture_id},{},{},{contract_sha256},{fixture_sha256},",
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{},{},",
                "{:.6},{:.6},{},{}\n"
            ),
            observation.case_id,
            fixture.layout.size.as_str(),
            observation.layout_checksum,
            observation.floors,
            observation.walls,
            observation.doors,
            fixture.layout.supplied_lamps.len(),
            1,
            observation.rooms,
            observation.room_tiles,
            observation.room_boundaries,
            observation.souls,
            observation.familiars,
            observation.yards,
            observation.soul_spas,
            observation.generator_souls,
            observation.main_generation,
            observation.main_demand,
            observation.main_headroom,
            observation.main_supplied_count,
            observation.main_shed_count,
            observation.control_generation,
            observation.control_demand,
            observation.control_supplied_count,
            observation.control_shed_count,
            contract_id = CONTRACT_ID,
            stage_id = stage_id,
            lane = lane,
            fixture_id = FIXTURE_ID,
            contract_sha256 = CONTRACT_SHA256,
            fixture_sha256 = FIXTURE_SHA256,
        );

        let mut layout_csv = String::from(
            "schema_version,record_kind,ordinal,grid_x,grid_y,grid_x2,grid_y2,state,relation\n",
        );
        for (ordinal, &grid) in fixture.layout.floors.iter().enumerate() {
            push_layout_row(
                &mut layout_csv,
                "floor",
                ordinal,
                grid,
                None,
                "completed",
                "",
            );
        }
        for (ordinal, &grid) in fixture.layout.walls.iter().enumerate() {
            push_layout_row(
                &mut layout_csv,
                "wall",
                ordinal,
                grid,
                None,
                "completed",
                "",
            );
        }
        for (ordinal, door) in fixture.layout.doors.iter().enumerate() {
            push_layout_row(
                &mut layout_csv,
                "door",
                ordinal,
                door.grid,
                None,
                door.state_name(),
                "",
            );
        }
        for (ordinal, &grid) in fixture.layout.supplied_lamps.iter().enumerate() {
            push_layout_row(
                &mut layout_csv,
                "supplied_lamp",
                ordinal,
                grid,
                None,
                "supplied",
                "main-yard",
            );
        }
        push_layout_row(
            &mut layout_csv,
            "unsupplied_lamp",
            0,
            fixture.layout.control_lamp,
            None,
            "shed-insufficient-generation",
            "control-yard",
        );
        let mut showcase_footprint_ordinal = 0;
        let mut companion_footprint_ordinal = 0;
        for (ordinal, showcase) in fixture.layout.showcase.iter().enumerate() {
            push_layout_row(
                &mut layout_csv,
                "showcase_building",
                ordinal,
                showcase.anchor,
                None,
                showcase.kind_name(),
                &showcase.relation(),
            );
            for &grid in &showcase.occupied_grids {
                push_layout_row(
                    &mut layout_csv,
                    "showcase_footprint",
                    showcase_footprint_ordinal,
                    grid,
                    None,
                    showcase.kind_name(),
                    &format!("showcase-building-{ordinal}"),
                );
                showcase_footprint_ordinal += 1;
            }
            if let Some(companion) = &showcase.companion {
                push_layout_row(
                    &mut layout_csv,
                    "showcase_companion",
                    0,
                    companion.anchor,
                    None,
                    "BucketStorage",
                    &format!("showcase-building-{ordinal}:tank-companion-placement"),
                );
                for &grid in &companion.occupied_grids {
                    push_layout_row(
                        &mut layout_csv,
                        "showcase_companion_footprint",
                        companion_footprint_ordinal,
                        grid,
                        None,
                        "BucketStorage",
                        "showcase-companion-0",
                    );
                    companion_footprint_ordinal += 1;
                }
            }
        }
        let worker_relations = fixture.layout.worker_relations();
        for (ordinal, &grid) in fixture.layout.soul_cells.iter().enumerate() {
            let relation = worker_relations.get(&ordinal).map_or("", String::as_str);
            push_layout_row(
                &mut layout_csv,
                "soul",
                ordinal,
                grid,
                None,
                if relation.is_empty() {
                    "idle"
                } else {
                    "generator"
                },
                relation,
            );
        }
        for (ordinal, &grid) in fixture.layout.familiar_cells.iter().enumerate() {
            push_layout_row(&mut layout_csv, "familiar", ordinal, grid, None, "idle", "");
        }
        push_layout_row(
            &mut layout_csv,
            "yard",
            0,
            fixture.layout.main_yard_bounds.0,
            Some(fixture.layout.main_yard_bounds.1),
            "main",
            "",
        );
        push_layout_row(
            &mut layout_csv,
            "yard",
            1,
            fixture.layout.control_yard_bounds.0,
            Some(fixture.layout.control_yard_bounds.1),
            "control",
            "",
        );
        for (spa_ordinal, spa) in fixture.layout.spas.iter().enumerate() {
            push_layout_row(
                &mut layout_csv,
                "soul_spa",
                spa_ordinal,
                spa.anchor,
                None,
                "operational",
                "main-yard",
            );
            for (tile_ordinal, &grid) in spa.tiles.iter().enumerate() {
                let relation = spa
                    .workers
                    .iter()
                    .find(|worker| worker.tile_ordinal == tile_ordinal)
                    .map_or_else(
                        || format!("soul-spa-{spa_ordinal}"),
                        |worker| format!("soul-{}", worker.soul_ordinal),
                    );
                push_layout_row(
                    &mut layout_csv,
                    "soul_spa_tile",
                    spa_ordinal * 4 + tile_ordinal,
                    grid,
                    None,
                    "generate-power",
                    &relation,
                );
            }
        }
        for (ordinal, &grid) in fixture.layout.floors.iter().enumerate() {
            push_layout_row(
                &mut layout_csv,
                "room_tile",
                ordinal,
                grid,
                None,
                "interior",
                &format!("room-{}", ordinal / ROOM_INTERIOR_TILES),
            );
        }
        for (ordinal, boundary) in fixture.layout.room_boundaries().into_iter().enumerate() {
            let relation = boundary
                .rooms
                .iter()
                .map(|room| format!("room-{room}"))
                .collect::<Vec<_>>()
                .join("+");
            push_layout_row(
                &mut layout_csv,
                "room_boundary",
                ordinal,
                boundary.grid,
                None,
                if boundary.is_door { "door" } else { "wall" },
                &relation,
            );
        }
        let mut presentation_csv = String::from(
            "schema_version,building_kind,entity_count,root_sprite_count,child_sprite_count,owner_3d_count\n",
        );
        for row in &observation.presentation {
            presentation_csv.push_str(&format!(
                "1,{},{},{},{},{}\n",
                row.building_kind,
                row.entity_count,
                row.root_sprite_count,
                row.child_sprite_count,
                row.owner_3d_count,
            ));
        }
        Ok((summary, layout_csv, presentation_csv))
    }
}

fn push_layout_row(
    csv: &mut String,
    record_kind: &str,
    ordinal: usize,
    grid: (i32, i32),
    grid2: Option<(i32, i32)>,
    state: &str,
    relation: &str,
) {
    let (grid_x2, grid_y2) = grid2
        .map(|(x, y)| (x.to_string(), y.to_string()))
        .unwrap_or_default();
    csv.push_str(&format!(
        "1,{record_kind},{ordinal},{},{},{grid_x2},{grid_y2},{state},{relation}\n",
        grid.0, grid.1
    ));
}

#[derive(Clone)]
struct IndoorLightFixtureEntities {
    layout: IndoorLightLayout,
    soul_entities: Vec<Entity>,
    familiar_entities: Vec<Entity>,
    main_yard: Entity,
    control_yard: Entity,
    spas: Vec<IndoorLightSpaEntities>,
    bucket_storages: Vec<Entity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IndoorLightSpaEntities {
    site: Entity,
    tiles: Vec<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DoorSpec {
    grid: Grid,
    state: hw_core::world::DoorState,
}

impl DoorSpec {
    const fn state_name(self) -> &'static str {
        match self.state {
            hw_core::world::DoorState::Open => "open",
            hw_core::world::DoorState::Closed => "closed",
            hw_core::world::DoorState::Locked => "locked",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GeneratorWorkerSpec {
    soul_ordinal: usize,
    tile_ordinal: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SoulSpaSpec {
    anchor: Grid,
    tiles: [Grid; 4],
    workers: Vec<GeneratorWorkerSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShowcaseCompanionSpec {
    anchor: Grid,
    occupied_grids: Vec<Grid>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShowcaseSource {
    Canonical(&'static str, usize, &'static str),
    Dedicated(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShowcaseSpec {
    kind: BuildingType,
    anchor: Grid,
    occupied_grids: Vec<Grid>,
    source: ShowcaseSource,
    companion: Option<ShowcaseCompanionSpec>,
}

impl ShowcaseSpec {
    const fn kind_name(&self) -> &'static str {
        match self.kind {
            BuildingType::Wall => "Wall",
            BuildingType::Door => "Door",
            BuildingType::Floor => "Floor",
            BuildingType::Tank => "Tank",
            BuildingType::MudMixer => "MudMixer",
            BuildingType::RestArea => "RestArea",
            BuildingType::Bridge => "Bridge",
            BuildingType::SandPile => "SandPile",
            BuildingType::BonePile => "BonePile",
            BuildingType::WheelbarrowParking => "WheelbarrowParking",
            BuildingType::SoulSpa => "SoulSpa",
            BuildingType::OutdoorLamp => "OutdoorLamp",
        }
    }

    fn relation(&self) -> String {
        match self.source {
            ShowcaseSource::Canonical(source, _, route) => format!("{source}:{route}"),
            ShowcaseSource::Dedicated(route) => format!("dedicated-showcase:{route}"),
        }
    }

    const fn is_dedicated(&self) -> bool {
        matches!(self.source, ShowcaseSource::Dedicated(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RoomBoundarySpec {
    grid: Grid,
    rooms: Vec<usize>,
    is_door: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IndoorLightLayout {
    size: PerfScenarioSize,
    layout_checksum: &'static str,
    module_count: usize,
    floors: Vec<Grid>,
    walls: Vec<Grid>,
    doors: Vec<DoorSpec>,
    supplied_lamps: Vec<Grid>,
    control_lamp: Grid,
    soul_cells: Vec<Grid>,
    familiar_cells: Vec<Grid>,
    spas: Vec<SoulSpaSpec>,
    showcase: Vec<ShowcaseSpec>,
    main_yard_bounds: (Grid, Grid),
    control_yard_bounds: (Grid, Grid),
}

impl IndoorLightLayout {
    fn build(size: PerfScenarioSize) -> Self {
        let (module_count, supplied_lamps, souls, familiars, layout_checksum) = match size {
            PerfScenarioSize::Small => (1, 1, 50, 4, SMALL_LAYOUT_SHA256),
            PerfScenarioSize::Medium => (2, 10, 200, 12, MEDIUM_LAYOUT_SHA256),
            PerfScenarioSize::Large => (4, 50, 500, 30, LARGE_LAYOUT_SHA256),
        };
        let floors = (0..module_count)
            .flat_map(|room_y| {
                (0..module_count).flat_map(move |room_x| {
                    (1..=6).flat_map(move |local_y| {
                        (1..=6).map(move |local_x| {
                            (
                                ORIGIN.0 + (room_x as i32 * MODULE_EXTENT) + local_x,
                                ORIGIN.1 + (room_y as i32 * MODULE_EXTENT) + local_y,
                            )
                        })
                    })
                })
            })
            .collect::<Vec<_>>();
        let extent = module_count as i32 * MODULE_EXTENT;
        let mut boundary = BTreeSet::new();
        for boundary_index in 0..=module_count {
            let line = boundary_index as i32 * MODULE_EXTENT;
            for offset in 0..=extent {
                boundary.insert((ORIGIN.0 + line, ORIGIN.1 + offset));
                boundary.insert((ORIGIN.0 + offset, ORIGIN.1 + line));
            }
        }
        let door_states = match size {
            PerfScenarioSize::Small => vec![hw_core::world::DoorState::Closed],
            PerfScenarioSize::Medium => vec![
                hw_core::world::DoorState::Open,
                hw_core::world::DoorState::Open,
                hw_core::world::DoorState::Closed,
                hw_core::world::DoorState::Locked,
            ],
            PerfScenarioSize::Large => (0..16)
                .map(|ordinal| match ordinal {
                    0..=3 => hw_core::world::DoorState::Open,
                    4..=11 => hw_core::world::DoorState::Closed,
                    _ => hw_core::world::DoorState::Locked,
                })
                .collect(),
        };
        let door_grids = (0..module_count)
            .flat_map(|room_y| {
                (0..module_count).map(move |room_x| {
                    (
                        ORIGIN.0 + room_x as i32 * MODULE_EXTENT + 3,
                        ORIGIN.1 + (room_y as i32 + 1) * MODULE_EXTENT,
                    )
                })
            })
            .collect::<Vec<_>>();
        let doors = door_grids
            .into_iter()
            .zip(door_states)
            .map(|(grid, state)| {
                boundary.remove(&grid);
                DoorSpec { grid, state }
            })
            .collect::<Vec<_>>();
        let soul_cells = (0..souls)
            .map(|index| floors[index % floors.len()])
            .collect();
        let familiar_cells = (0..familiars)
            .map(|index| floors[(18 + index) % floors.len()])
            .collect();
        let spas = Self::spa_specs(size);
        let showcase = Self::showcase_specs(size, &spas);
        let control_lamp = (
            match size {
                PerfScenarioSize::Small => 80,
                PerfScenarioSize::Medium => 82,
                PerfScenarioSize::Large => 84,
            },
            80,
        );
        Self {
            size,
            layout_checksum,
            module_count,
            supplied_lamps: floors[..supplied_lamps].to_vec(),
            control_lamp,
            floors,
            walls: boundary.into_iter().collect(),
            doors,
            soul_cells,
            familiar_cells,
            spas,
            showcase,
            main_yard_bounds: (ORIGIN, (ORIGIN.0 + extent, ORIGIN.1 + extent)),
            control_yard_bounds: (control_lamp, control_lamp),
        }
    }

    fn spa_specs(size: PerfScenarioSize) -> Vec<SoulSpaSpec> {
        let spa = |anchor, workers: &[(usize, usize)]| SoulSpaSpec {
            anchor,
            tiles: [
                anchor,
                (anchor.0 + 1, anchor.1),
                (anchor.0, anchor.1 - 1),
                (anchor.0 + 1, anchor.1 - 1),
            ],
            workers: workers
                .iter()
                .map(|&(soul_ordinal, tile_ordinal)| GeneratorWorkerSpec {
                    soul_ordinal,
                    tile_ordinal,
                })
                .collect(),
        };
        match size {
            PerfScenarioSize::Small => vec![spa((21, 26), &[(28, 2)])],
            PerfScenarioSize::Medium => vec![spa((27, 25), &[(57, 2), (58, 3), (63, 0)])],
            PerfScenarioSize::Large => vec![
                spa((20, 39), &[(309, 2), (310, 3), (315, 0), (316, 1)]),
                spa((27, 39), &[(345, 2), (346, 3), (351, 0), (352, 1)]),
                spa((34, 39), &[(381, 2), (382, 3), (387, 0)]),
            ],
        }
    }

    fn showcase_specs(size: PerfScenarioSize, spas: &[SoulSpaSpec]) -> Vec<ShowcaseSpec> {
        if size == PerfScenarioSize::Small {
            return Vec::new();
        }
        let geometry = |kind, anchor| {
            hw_ui::selection::building_geometry(kind, anchor, RIVER_Y_MIN).occupied_grids
        };
        let canonical = |kind, anchor, source, ordinal, route| ShowcaseSpec {
            kind,
            anchor,
            occupied_grids: geometry(kind, anchor),
            source: ShowcaseSource::Canonical(source, ordinal, route),
            companion: None,
        };
        let dedicated = |kind, anchor, route| ShowcaseSpec {
            kind,
            anchor,
            occupied_grids: geometry(kind, anchor),
            source: ShowcaseSource::Dedicated(route),
            companion: None,
        };
        vec![
            canonical(
                BuildingType::Wall,
                (16, 20),
                "canonical-wall",
                0,
                "area-wall-completion",
            ),
            canonical(
                BuildingType::Door,
                (19, 27),
                "canonical-door",
                0,
                "completed-blueprint",
            ),
            canonical(
                BuildingType::Floor,
                (17, 21),
                "canonical-floor",
                0,
                "area-floor-completion",
            ),
            ShowcaseSpec {
                companion: Some(ShowcaseCompanionSpec {
                    anchor: (17, 30),
                    occupied_grids: vec![(17, 30), (18, 30)],
                }),
                ..dedicated(BuildingType::Tank, (17, 28), "completed-blueprint")
            },
            dedicated(BuildingType::MudMixer, (20, 28), "completed-blueprint"),
            dedicated(BuildingType::RestArea, (17, 31), "completed-blueprint"),
            dedicated(
                BuildingType::Bridge,
                (90, 65),
                "fixture-seeded-completed-blueprint",
            ),
            dedicated(BuildingType::SandPile, (27, 28), "completed-blueprint"),
            dedicated(BuildingType::BonePile, (28, 28), "completed-blueprint"),
            dedicated(
                BuildingType::WheelbarrowParking,
                (24, 28),
                "completed-blueprint",
            ),
            canonical(
                BuildingType::SoulSpa,
                spas[0].anchor,
                "canonical-soul-spa",
                0,
                "soul-spa-placement",
            ),
            canonical(
                BuildingType::OutdoorLamp,
                (17, 21),
                "canonical-supplied-lamp",
                0,
                "completed-blueprint",
            ),
        ]
    }

    fn occupied_cells(&self) -> BTreeMap<Grid, bool> {
        let mut cells = BTreeMap::new();
        let mut insert = |grid, walkable_required| {
            cells
                .entry(grid)
                .and_modify(|required| *required |= walkable_required)
                .or_insert(walkable_required);
        };
        for &grid in self
            .floors
            .iter()
            .chain(&self.walls)
            .chain(self.doors.iter().map(|door| &door.grid))
            .chain(&self.supplied_lamps)
            .chain(std::iter::once(&self.control_lamp))
        {
            insert(grid, true);
        }
        for spa in &self.spas {
            for &grid in &spa.tiles {
                insert(grid, true);
            }
        }
        for showcase in &self.showcase {
            for &grid in &showcase.occupied_grids {
                insert(grid, showcase.kind != BuildingType::Bridge);
            }
            if let Some(companion) = &showcase.companion {
                for &grid in &companion.occupied_grids {
                    insert(grid, true);
                }
            }
        }
        cells
    }

    fn main_yard(&self) -> Yard {
        Yard {
            min: WorldMap::grid_to_world(self.main_yard_bounds.0.0, self.main_yard_bounds.0.1),
            max: WorldMap::grid_to_world(self.main_yard_bounds.1.0, self.main_yard_bounds.1.1),
        }
    }

    fn control_yard(&self) -> Yard {
        let position = WorldMap::grid_to_world(self.control_lamp.0, self.control_lamp.1);
        Yard {
            min: position,
            max: position,
        }
    }

    fn worker_relations(&self) -> BTreeMap<usize, String> {
        self.spas
            .iter()
            .enumerate()
            .flat_map(|(spa_ordinal, spa)| {
                spa.workers.iter().map(move |worker| {
                    (
                        worker.soul_ordinal,
                        format!("soul-spa-{spa_ordinal}-tile-{}", worker.tile_ordinal),
                    )
                })
            })
            .collect()
    }

    fn room_boundaries(&self) -> Vec<RoomBoundarySpec> {
        let floor_rooms = self
            .floors
            .iter()
            .enumerate()
            .map(|(ordinal, &grid)| (grid, ordinal / ROOM_INTERIOR_TILES))
            .collect::<BTreeMap<_, _>>();
        let door_cells = self
            .doors
            .iter()
            .map(|door| door.grid)
            .collect::<HashSet<_>>();
        let boundary_grids = self
            .walls
            .iter()
            .copied()
            .chain(self.doors.iter().map(|door| door.grid))
            .filter(|grid| {
                [(0, 1), (1, 0), (0, -1), (-1, 0)]
                    .into_iter()
                    .any(|(dx, dy)| floor_rooms.contains_key(&(grid.0 + dx, grid.1 + dy)))
            })
            .collect::<BTreeSet<_>>();
        boundary_grids
            .into_iter()
            .map(|grid| {
                let rooms = [(0, 1), (1, 0), (0, -1), (-1, 0)]
                    .into_iter()
                    .filter_map(|(dx, dy)| floor_rooms.get(&(grid.0 + dx, grid.1 + dy)).copied())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                RoomBoundarySpec {
                    grid,
                    rooms,
                    is_door: door_cells.contains(&grid),
                }
            })
            .collect()
    }

    fn generator_count(&self) -> usize {
        self.spas.iter().map(|spa| spa.workers.len()).sum()
    }
}

pub(crate) fn should_settle_indoor_light_fixture(
    config: Res<PerfScenarioConfig>,
    state: Res<IndoorLightFixtureState>,
) -> bool {
    config.enabled()
        && config.workload == PerfWorkload::IndoorLight
        && state.phase == IndoorLightFixturePhase::Settling
}

pub(super) struct IndoorLightFixtureSetupContext<'a, 'w, 's> {
    pub(super) commands: &'a mut Commands<'w, 's>,
    pub(super) state: &'a mut IndoorLightFixtureState,
    pub(super) q_familiars: &'a mut PerfSetupFamiliarQuery<'w, 's>,
    pub(super) q_souls: &'a mut PerfSetupSoulQuery<'w, 's>,
    pub(super) world_map: &'a mut WorldMapWrite<'w>,
    pub(super) q_existing_yards: &'a Query<'w, 's, &'static Yard>,
    pub(super) game_assets: &'a GameAssets,
    pub(super) handles_3d: &'a Building3dHandles,
    pub(super) exit: &'a mut MessageWriter<'w, AppExit>,
}

pub(super) fn begin_indoor_light_fixture(
    config: &PerfScenarioConfig,
    context: IndoorLightFixtureSetupContext<'_, '_, '_>,
) {
    let IndoorLightFixtureSetupContext {
        commands,
        state,
        q_familiars,
        q_souls,
        world_map,
        q_existing_yards,
        game_assets,
        handles_3d,
        exit,
    } = context;
    if state.phase != IndoorLightFixturePhase::Inactive {
        return;
    }
    let layout = IndoorLightLayout::build(config.size);
    for spa in &layout.spas {
        let geometry =
            hw_ui::selection::building_geometry(BuildingType::SoulSpa, spa.anchor, RIVER_Y_MIN);
        if geometry.occupied_grids != spa.tiles {
            fail_fixture(
                state,
                exit,
                format!(
                    "production SoulSpa geometry {:?} differs from contract {:?}",
                    geometry.occupied_grids, spa.tiles
                ),
            );
            return;
        }
    }
    if !layout.showcase.is_empty()
        && layout
            .showcase
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>()
            != BuildingType::ALL
    {
        fail_fixture(
            state,
            exit,
            "all-building showcase does not enumerate BuildingType::ALL".to_string(),
        );
        return;
    }
    if let Some((grid, walkable_required)) =
        layout
            .occupied_cells()
            .into_iter()
            .find(|(grid, required)| {
                grid.0 < 0
                    || grid.0 >= hw_core::constants::MAP_WIDTH
                    || grid.1 < 0
                    || grid.1 >= hw_core::constants::MAP_HEIGHT
                    || (*required && !world_map.is_walkable(grid.0, grid.1))
                    || world_map.has_building(*grid)
                    || world_map.door_entity(grid.0, grid.1).is_some()
                    || world_map.stockpile_entity(*grid).is_some()
            })
    {
        fail_fixture(
            state,
            exit,
            format!(
                "canonical indoor-light cell {grid:?} is not empty{}; refusing to delete world-generation content",
                if walkable_required {
                    " and terrain-walkable"
                } else {
                    ""
                }
            ),
        );
        return;
    }
    if let Some(grid) = layout.occupied_cells().into_keys().find(|&(x, y)| {
        let position = WorldMap::grid_to_world(x, y);
        q_existing_yards.iter().any(|yard| yard.contains(position))
    }) {
        fail_fixture(
            state,
            exit,
            format!(
                "canonical indoor-light cell {grid:?} overlaps a pre-existing Yard; refusing ambiguous power topology"
            ),
        );
        return;
    }

    let mut soul_entities = q_souls
        .iter()
        .map(|(entity, ..)| entity)
        .collect::<Vec<_>>();
    soul_entities.sort_unstable_by_key(|entity| entity.to_bits());
    let mut familiar_entities = q_familiars
        .iter()
        .map(|(entity, ..)| entity)
        .collect::<Vec<_>>();
    familiar_entities.sort_unstable_by_key(|entity| entity.to_bits());
    if soul_entities.len() != layout.soul_cells.len()
        || familiar_entities.len() != layout.familiar_cells.len()
    {
        fail_fixture(
            state,
            exit,
            format!(
                "indoor-light actor population is {}/{}; expected {}/{}",
                soul_entities.len(),
                familiar_entities.len(),
                layout.soul_cells.len(),
                layout.familiar_cells.len()
            ),
        );
        return;
    }

    let completed_floors = layout
        .floors
        .iter()
        .copied()
        .map(|grid| (spawn_completed_floor_tile(commands, handles_3d, grid), grid))
        .collect::<Vec<_>>();
    register_completed_floors(world_map, &completed_floors);

    for &grid in &layout.walls {
        let wall = spawn_wall_shell(commands, handles_3d, grid, false);
        world_map.reserve_building_footprint(BuildingType::Wall, wall, std::iter::once(grid));
    }
    for door in &layout.doors {
        spawn_completed_blueprint(commands, world_map, BuildingType::Door, door.grid);
    }
    for &grid in &layout.supplied_lamps {
        spawn_completed_blueprint(commands, world_map, BuildingType::OutdoorLamp, grid);
    }
    spawn_completed_blueprint(
        commands,
        world_map,
        BuildingType::OutdoorLamp,
        layout.control_lamp,
    );

    let mut bucket_storages = Vec::new();
    for showcase in layout.showcase.iter().filter(|entry| entry.is_dedicated()) {
        let blueprint =
            spawn_completed_blueprint(commands, world_map, showcase.kind, showcase.anchor);
        if let Some(companion) = &showcase.companion {
            match try_place_bucket_storage_companion(
                commands,
                world_map,
                blueprint,
                &showcase.occupied_grids,
                companion.anchor,
            ) {
                Ok(entities) => bucket_storages.extend(entities),
                Err(rejection) => {
                    fail_fixture(
                        state,
                        exit,
                        format!(
                            "Tank companion placement at {:?} was rejected: {:?}",
                            companion.anchor, rejection.reason
                        ),
                    );
                    return;
                }
            }
        }
    }

    let main_yard = commands
        .spawn((Name::new("PerfIndoorLightMainYard"), layout.main_yard()))
        .id();
    let control_yard = commands
        .spawn((
            Name::new("PerfIndoorLightControlYard"),
            layout.control_yard(),
        ))
        .id();

    let spas = layout
        .spas
        .iter()
        .map(|spa| {
            let geometry =
                hw_ui::selection::building_geometry(BuildingType::SoulSpa, spa.anchor, RIVER_Y_MIN);
            let (site, tiles) = spawn_soul_spa(
                commands,
                world_map,
                &geometry.occupied_grids,
                geometry.draw_pos,
                None,
                game_assets,
                handles_3d,
            );
            IndoorLightSpaEntities { site, tiles }
        })
        .collect();

    state.fixture = Some(IndoorLightFixtureEntities {
        layout,
        soul_entities,
        familiar_entities,
        main_yard,
        control_yard,
        spas,
        bucket_storages,
    });
    state.door_states_seeded = false;
    state.phase = IndoorLightFixturePhase::Settling;
}

fn spawn_completed_blueprint(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    kind: BuildingType,
    grid: (i32, i32),
) -> Entity {
    let geometry = hw_ui::selection::building_geometry(kind, grid, RIVER_Y_MIN);
    let mut blueprint = Blueprint::new(kind, geometry.occupied_grids.clone());
    for (resource, required) in blueprint.required_materials.clone() {
        blueprint.deliver_material(resource, required);
    }
    if blueprint.flexible_material_requirement.is_some() {
        let required = blueprint.remaining_material_amount(ResourceType::Wood);
        blueprint.deliver_material(ResourceType::Wood, required);
    }
    blueprint.progress = 1.0;
    let entity = commands
        .spawn((
            blueprint,
            Transform::from_translation(geometry.draw_pos.extend(hw_core::constants::Z_MAP)),
            Name::new(format!("PerfIndoorLightBlueprint ({kind:?})")),
        ))
        .id();
    world_map.reserve_building_footprint(kind, entity, geometry.occupied_grids);
    entity
}

pub(crate) fn stabilize_indoor_light_actors_system(
    mut commands: Commands,
    state: Res<IndoorLightFixtureState>,
    mut q_souls: PerfSetupSoulQuery,
    mut q_familiars: PerfSetupFamiliarQuery,
) {
    let Some(fixture) = state.fixture.as_ref() else {
        return;
    };
    for (&entity, &grid) in fixture.soul_entities.iter().zip(&fixture.layout.soul_cells) {
        let Ok((_, mut transform, mut destination, mut path, mut task)) = q_souls.get_mut(entity)
        else {
            continue;
        };
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        destination.0 = position;
        path.waypoints.clear();
        path.current_index = 0;
        path.planned_destination = None;
        *task = AssignedTask::None;
        commands
            .entity(entity)
            .remove::<(WorkingOn, ActiveTaskIdentity)>();
    }
    for (&entity, &grid) in fixture
        .familiar_entities
        .iter()
        .zip(&fixture.layout.familiar_cells)
    {
        let Ok((_, transform, mut command, mut operation, mut policy)) =
            q_familiars.get_mut(entity)
        else {
            continue;
        };
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
        *policy = FamiliarPolicy::default();
        let mut next_transform = *transform;
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        next_transform.translation.x = position.x;
        next_transform.translation.y = position.y;
        commands.entity(entity).insert(next_transform);
    }
}

pub(crate) fn seed_indoor_light_static_door_states_system(
    mut state: ResMut<IndoorLightFixtureState>,
    mut world_map: WorldMapWrite,
    handles: Res<DoorVisualHandles>,
    mut q_doors: Query<(Entity, &Transform, &mut Door, &Children)>,
    mut q_sprites: Query<&mut Sprite>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.door_states_seeded {
        return;
    }
    let Some(layout) = state.fixture.as_ref().map(|fixture| fixture.layout.clone()) else {
        return;
    };
    for expected in &layout.doors {
        let expected_pos = WorldMap::grid_to_world(expected.grid.0, expected.grid.1);
        let mut matches = q_doors
            .iter_mut()
            .filter(|(_, transform, ..)| transform.translation.truncate() == expected_pos);
        let Some((entity, _, mut door, children)) = matches.next() else {
            fail_fixture(
                &mut state,
                &mut exit,
                format!(
                    "Door at {:?} is missing during static state seed",
                    expected.grid
                ),
            );
            return;
        };
        if matches.next().is_some()
            || world_map.door_entity(expected.grid.0, expected.grid.1) != Some(entity)
        {
            fail_fixture(
                &mut state,
                &mut exit,
                format!(
                    "Door at {:?} is duplicated or has a different WorldMap owner",
                    expected.grid
                ),
            );
            return;
        }
        let sprite_children = children
            .iter()
            .filter(|child| q_sprites.contains(*child))
            .collect::<Vec<_>>();
        let [sprite_entity] = sprite_children.as_slice() else {
            fail_fixture(
                &mut state,
                &mut exit,
                format!(
                    "Door at {:?} does not have exactly one child Sprite",
                    expected.grid
                ),
            );
            return;
        };
        let Ok(mut sprite) = q_sprites.get_mut(*sprite_entity) else {
            fail_fixture(
                &mut state,
                &mut exit,
                format!("Door child Sprite at {:?} vanished", expected.grid),
            );
            return;
        };
        hw_world::apply_door_state(
            &mut door,
            &mut sprite,
            &mut world_map,
            &handles,
            expected.grid,
            expected.state,
        );
    }
    state.door_states_seeded = true;
}

pub(crate) fn prepare_indoor_light_soul_spa_system(
    state: Res<IndoorLightFixtureState>,
    mut q_sites: Query<&mut SoulSpaSite>,
) {
    let Some(fixture) = state.fixture.as_ref() else {
        return;
    };
    for spa in &fixture.spas {
        let Ok(mut site) = q_sites.get_mut(spa.site) else {
            continue;
        };
        site.phase = SoulSpaPhase::Operational;
        site.bones_delivered = site.bones_required;
    }
}

pub(crate) fn assign_indoor_light_generator_system(
    mut commands: Commands,
    state: Res<IndoorLightFixtureState>,
    mut q_souls: Query<
        (
            &mut DamnedSoul,
            &mut Transform,
            &mut Destination,
            &mut Path,
            &mut AssignedTask,
        ),
        With<DamnedSoul>,
    >,
    q_tiles: Query<&SoulSpaTile>,
) {
    let Some(fixture) = state.fixture.as_ref() else {
        return;
    };
    for (spa_spec, spa_entities) in fixture.layout.spas.iter().zip(&fixture.spas) {
        for worker in &spa_spec.workers {
            let tile_entity = spa_entities.tiles[worker.tile_ordinal];
            let Ok(tile) = q_tiles.get(tile_entity) else {
                continue;
            };
            let tile_pos = WorldMap::grid_to_world(tile.grid_pos.0, tile.grid_pos.1);
            let soul_entity = fixture.soul_entities[worker.soul_ordinal];
            let Ok((mut soul, mut transform, mut destination, mut path, mut task)) =
                q_souls.get_mut(soul_entity)
            else {
                continue;
            };
            soul.dream = 100.0;
            transform.translation.x = tile_pos.x;
            transform.translation.y = tile_pos.y;
            destination.0 = tile_pos;
            path.waypoints.clear();
            path.current_index = 0;
            path.planned_destination = None;
            *task = AssignedTask::GeneratePower(GeneratePowerData {
                tile: tile_entity,
                tile_pos,
                phase: GeneratePowerPhase::Generating,
            });
            commands.entity(soul_entity).insert((
                WorkingOn(tile_entity),
                ActiveTaskIdentity::new(tile_entity, tile_entity, WorkType::GeneratePower),
            ));
        }
    }
}

type IndoorLightBuildingQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Building,
        &'static Transform,
        Option<&'static Door>,
        Option<&'static PowerConsumer>,
        Option<&'static PowerSupplyState>,
        Option<&'static ConsumesFrom>,
        Has<Unpowered>,
        Option<&'static Children>,
    ),
>;
type IndoorLightGridQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PowerGrid,
        &'static YardPowerGrid,
        Option<&'static PowerGridAllocationSummary>,
        Option<&'static GridGenerators>,
        Option<&'static GridConsumers>,
    ),
>;
type IndoorLightSpaQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SoulSpaSite,
        &'static PowerGenerator,
        Option<&'static GeneratesFor>,
    ),
>;
type IndoorLightTileQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SoulSpaTile,
        Option<&'static Designation>,
        Option<&'static TaskSlots>,
        Option<&'static TaskWorkers>,
    ),
>;
type IndoorLightSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static AssignedTask,
        Option<&'static WorkingOn>,
        Option<&'static ActiveTaskIdentity>,
        &'static DamnedSoul,
    ),
    PerfSetupSoulFilter,
>;
type IndoorLightBucketStorageQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Stockpile,
        &'static BelongsTo,
        Option<&'static PendingBelongsToBlueprint>,
        &'static Transform,
    ),
    With<BucketStorage>,
>;
type IndoorLightOwnedItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ResourceItem,
        &'static BelongsTo,
        Option<&'static StoredIn>,
        &'static TaskSlots,
        &'static Transform,
        Has<Wheelbarrow>,
    ),
>;

#[derive(SystemParam)]
pub(super) struct IndoorLightAuditQueries<'w, 's> {
    state: Res<'w, IndoorLightFixtureState>,
    q_buildings: IndoorLightBuildingQuery<'w, 's>,
    q_sprites: Query<'w, 's, &'static Sprite>,
    q_visuals: Query<'w, 's, &'static Building3dVisual>,
    q_yards: Query<'w, 's, &'static Yard>,
    q_grids: IndoorLightGridQuery<'w, 's>,
    q_spas: IndoorLightSpaQuery<'w, 's>,
    q_tiles: IndoorLightTileQuery<'w, 's>,
    q_rooms: Query<'w, 's, (Entity, &'static Room)>,
    room_tiles: Res<'w, RoomTileLookup>,
    room_boundaries: Res<'w, RoomBoundaryLookup>,
    world_map: Res<'w, WorldMap>,
    q_stockpiles: Query<'w, 's, &'static Stockpile>,
    q_bucket_storages: IndoorLightBucketStorageQuery<'w, 's>,
    q_owned_items: IndoorLightOwnedItemsQuery<'w, 's>,
    q_mud_mixers: Query<'w, 's, (), With<MudMixerStorage>>,
    q_rest_areas: Query<'w, 's, (), With<RestArea>>,
    q_bridges: Query<'w, 's, (), With<BridgeMarker>>,
    q_sand_piles: Query<'w, 's, (), With<SandPile>>,
    q_bone_piles: Query<'w, 's, (), With<BonePile>>,
    q_wheelbarrow_parking: Query<'w, 's, &'static WheelbarrowParking>,
    door_handles: Res<'w, DoorVisualHandles>,
}

#[derive(SystemParam)]
pub(crate) struct IndoorLightValidationParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    state: ResMut<'w, IndoorLightFixtureState>,
    applied: ResMut<'w, PerfScenarioApplied>,
    exit: MessageWriter<'w, AppExit>,
    q_buildings: IndoorLightBuildingQuery<'w, 's>,
    q_sprites: Query<'w, 's, &'static Sprite>,
    q_visuals: Query<'w, 's, &'static Building3dVisual>,
    q_yards: Query<'w, 's, &'static Yard>,
    q_grids: IndoorLightGridQuery<'w, 's>,
    q_spas: IndoorLightSpaQuery<'w, 's>,
    q_tiles: IndoorLightTileQuery<'w, 's>,
    q_souls: IndoorLightSoulQuery<'w, 's>,
    q_familiars: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static ActiveCommand,
            &'static FamiliarOperation,
        ),
        PerfSetupFamiliarFilter,
    >,
    q_rooms: Query<'w, 's, (Entity, &'static Room)>,
    room_tiles: Res<'w, RoomTileLookup>,
    room_boundaries: Res<'w, RoomBoundaryLookup>,
    world_map: Res<'w, WorldMap>,
    q_stockpiles: Query<'w, 's, &'static Stockpile>,
    q_bucket_storages: IndoorLightBucketStorageQuery<'w, 's>,
    q_owned_items: IndoorLightOwnedItemsQuery<'w, 's>,
    q_mud_mixers: Query<'w, 's, (), With<MudMixerStorage>>,
    q_rest_areas: Query<'w, 's, (), With<RestArea>>,
    q_bridges: Query<'w, 's, (), With<BridgeMarker>>,
    q_sand_piles: Query<'w, 's, (), With<SandPile>>,
    q_bone_piles: Query<'w, 's, (), With<BonePile>>,
    q_wheelbarrow_parking: Query<'w, 's, &'static WheelbarrowParking>,
    door_handles: Res<'w, DoorVisualHandles>,
}

#[derive(Clone)]
struct ObservedBuilding {
    entity: Entity,
    kind: BuildingType,
    draw_pos: Vec2,
    door_state: Option<hw_core::world::DoorState>,
    demand: Option<f32>,
    supply: Option<PowerSupplyState>,
    consumes_from: Option<Entity>,
    unpowered: bool,
    root_sprite: bool,
    child_sprites: usize,
    child_images: Vec<Handle<Image>>,
    owner_visuals: usize,
}

pub(crate) fn validate_indoor_light_fixture_system(mut p: IndoorLightValidationParams) {
    let Some(fixture) = p.state.fixture.clone() else {
        return;
    };
    let observed_buildings = p
        .q_buildings
        .iter()
        .map(
            |(
                entity,
                building,
                transform,
                door,
                consumer,
                supply,
                consumes_from,
                unpowered,
                children,
            )| ObservedBuilding {
                entity,
                kind: building.kind,
                draw_pos: transform.translation.truncate(),
                door_state: door.map(|door| door.state),
                demand: consumer.map(|consumer| consumer.demand),
                supply: supply.copied(),
                consumes_from: consumes_from.map(|relation| relation.0),
                unpowered,
                root_sprite: p.q_sprites.contains(entity),
                child_sprites: children.map_or(0, |children| {
                    children
                        .iter()
                        .filter(|child| p.q_sprites.contains(*child))
                        .count()
                }),
                child_images: children.map_or_else(Vec::new, |children| {
                    children
                        .iter()
                        .filter_map(|child| p.q_sprites.get(child).ok())
                        .map(|sprite| sprite.image.clone())
                        .collect()
                }),
                owner_visuals: p
                    .q_visuals
                    .iter()
                    .filter(|visual| visual.owner == entity)
                    .count(),
            },
        )
        .collect::<Vec<_>>();

    let validation = validate_observed_fixture(&fixture, &observed_buildings, &p);
    match validation {
        Ok((observation, audit_entities)) => {
            let layout_checksum = observation.layout_checksum;
            p.state.observation = Some(observation);
            p.state.audit_entities = Some(audit_entities);
            p.state.phase = IndoorLightFixturePhase::Ready;
            p.applied.workload = true;
            info!(
                "PERF_CAPTURE: indoor-light {}/current/static fixture settled ({})",
                p.config.size.as_str(),
                layout_checksum,
            );
        }
        Err(reason) => fail_fixture(&mut p.state, &mut p.exit, reason),
    }
}

fn validate_observed_fixture(
    fixture: &IndoorLightFixtureEntities,
    buildings: &[ObservedBuilding],
    p: &IndoorLightValidationParams,
) -> Result<(IndoorLightFixtureObservation, IndoorLightAuditEntities), String> {
    let layout = &fixture.layout;
    let exact_building = |kind, anchor| -> Result<&ObservedBuilding, String> {
        let draw_pos = hw_ui::selection::building_geometry(kind, anchor, RIVER_Y_MIN).draw_pos;
        let matches = buildings
            .iter()
            .filter(|building| building.kind == kind && building.draw_pos == draw_pos)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "expected one {kind:?} at anchor {anchor:?}, observed {}",
                matches.len()
            ));
        }
        Ok(matches[0])
    };
    let assert_presentation = |building: &ObservedBuilding| -> Result<(), String> {
        let (child_sprites, owner_visuals) = expected_presentation(building.kind);
        if building.root_sprite
            || building.child_sprites != child_sprites
            || building.owner_visuals != owner_visuals
        {
            return Err(format!(
                "{:?} at {:?} presentation is root Sprite={}, child Sprite={}, owner 3D={}; expected false/{child_sprites}/{owner_visuals}",
                building.kind,
                building.draw_pos,
                building.root_sprite,
                building.child_sprites,
                building.owner_visuals
            ));
        }
        Ok(())
    };

    let floors = fixture
        .layout
        .floors
        .iter()
        .map(|&grid| exact_building(BuildingType::Floor, grid))
        .collect::<Result<Vec<_>, _>>()?;
    for floor in &floors {
        assert_presentation(floor)?;
    }
    let walls = fixture
        .layout
        .walls
        .iter()
        .map(|&grid| exact_building(BuildingType::Wall, grid))
        .collect::<Result<Vec<_>, _>>()?;
    for wall in &walls {
        assert_presentation(wall)?;
    }
    let doors = layout
        .doors
        .iter()
        .map(|spec| {
            let building = exact_building(BuildingType::Door, spec.grid)?;
            assert_presentation(building)?;
            let expected_image = if spec.state == hw_core::world::DoorState::Open {
                &p.door_handles.door_open
            } else {
                &p.door_handles.door_closed
            };
            if building.door_state != Some(spec.state)
                || building.child_images != [expected_image.clone()]
                || p.world_map.door_entity(spec.grid.0, spec.grid.1) != Some(building.entity)
                || p.world_map.door_state(spec.grid.0, spec.grid.1) != Some(spec.state)
                || p.world_map.is_walkable(spec.grid.0, spec.grid.1)
                    != (spec.state != hw_core::world::DoorState::Locked)
            {
                return Err(format!(
                    "Door at {:?} differs from static {:?} state/image/WorldMap contract",
                    spec.grid, spec.state
                ));
            }
            Ok(building)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let supplied_lamps = layout
        .supplied_lamps
        .iter()
        .map(|&grid| exact_building(BuildingType::OutdoorLamp, grid))
        .collect::<Result<Vec<_>, _>>()?;
    let control_lamp = exact_building(BuildingType::OutdoorLamp, layout.control_lamp)?;
    for lamp in supplied_lamps.iter().copied().chain([control_lamp]) {
        assert_presentation(lamp)?;
        if !lamp
            .demand
            .is_some_and(|demand| close(demand, hw_energy::OUTDOOR_LAMP_DEMAND))
        {
            return Err(format!("Lamp at {:?} has invalid demand", lamp.draw_pos));
        }
    }
    let soul_spas = layout
        .spas
        .iter()
        .zip(&fixture.spas)
        .map(|(spec, entities)| {
            let building = exact_building(BuildingType::SoulSpa, spec.anchor)?;
            assert_presentation(building)?;
            if building.entity != entities.site {
                return Err(format!(
                    "SoulSpa at {:?} differs from the production spawn result",
                    spec.anchor
                ));
            }
            Ok(building)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let dedicated_showcase = layout
        .showcase
        .iter()
        .filter(|entry| entry.is_dedicated())
        .map(|entry| {
            let building = exact_building(entry.kind, entry.anchor)?;
            assert_presentation(building)?;
            Ok((entry, building))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let expected_building_count = floors.len()
        + walls.len()
        + doors.len()
        + supplied_lamps.len()
        + 1
        + soul_spas.len()
        + dedicated_showcase.len();
    let fixture_building_entities = floors
        .iter()
        .chain(&walls)
        .chain(&doors)
        .chain(&supplied_lamps)
        .copied()
        .chain(std::iter::once(control_lamp))
        .chain(soul_spas.iter().copied())
        .chain(dedicated_showcase.iter().map(|(_, building)| *building))
        .map(|building| building.entity)
        .collect::<HashSet<_>>();
    if fixture_building_entities.len() != expected_building_count {
        return Err(format!(
            "indoor-light fixture resolves to {} distinct Building entities, expected {expected_building_count}",
            fixture_building_entities.len()
        ));
    }

    let main_yard = p
        .q_yards
        .get(fixture.main_yard)
        .map_err(|_| "main fixture Yard is missing".to_string())?;
    let control_yard = p
        .q_yards
        .get(fixture.control_yard)
        .map_err(|_| "control fixture Yard is missing".to_string())?;
    if main_yard.min != layout.main_yard().min
        || main_yard.max != layout.main_yard().max
        || control_yard.min != layout.control_yard().min
        || control_yard.max != layout.control_yard().max
    {
        return Err("fixture Yard bounds differ from the contract".to_string());
    }

    let grids = p.q_grids.iter().collect::<Vec<_>>();
    let exact_grid = |yard| {
        let matches = grids
            .iter()
            .filter(|(_, _, owner, ..)| owner.0 == yard)
            .copied()
            .collect::<Vec<_>>();
        (matches.len() == 1).then_some(matches[0])
    };
    let (main_grid_entity, main_grid, _, main_summary, main_generators, main_consumers) =
        exact_grid(fixture.main_yard)
            .ok_or_else(|| "main Yard must own exactly one PowerGrid".to_string())?;
    let (
        control_grid_entity,
        control_grid,
        _,
        control_summary,
        control_generators,
        control_consumers,
    ) = exact_grid(fixture.control_yard)
        .ok_or_else(|| "control Yard must own exactly one PowerGrid".to_string())?;
    let spa_entity_set = fixture
        .spas
        .iter()
        .map(|spa| spa.site)
        .collect::<HashSet<_>>();
    let supplied_lamp_set = supplied_lamps
        .iter()
        .map(|lamp| lamp.entity)
        .collect::<HashSet<_>>();
    if main_generators.map_or(0, GridGenerators::len) != spa_entity_set.len()
        || !main_generators
            .is_some_and(|items| items.iter().copied().collect::<HashSet<_>>() == spa_entity_set)
        || main_consumers.map_or(0, GridConsumers::len) != supplied_lamp_set.len()
        || !main_consumers
            .is_some_and(|items| items.iter().copied().collect::<HashSet<_>>() == supplied_lamp_set)
    {
        return Err(
            "main PowerGrid generator/consumer topology differs from the contract".to_string(),
        );
    }
    if control_generators.is_some_and(|items| !items.is_empty())
        || control_consumers.map_or(0, GridConsumers::len) != 1
        || !control_consumers
            .is_some_and(|items| items.iter().any(|item| *item == control_lamp.entity))
    {
        return Err(
            "control PowerGrid topology differs from the generator-less contract".to_string(),
        );
    }
    for lamp in &supplied_lamps {
        if lamp.supply != Some(PowerSupplyState::Supplied)
            || lamp.unpowered
            || lamp.consumes_from != Some(main_grid_entity)
        {
            return Err("main Lamp is not production-supplied by the main grid".to_string());
        }
    }
    if control_lamp.supply
        != Some(PowerSupplyState::Shed {
            reason: PowerShedReason::InsufficientGeneration,
        })
        || !control_lamp.unpowered
        || control_lamp.consumes_from != Some(control_grid_entity)
    {
        return Err("control Lamp must be connected and Shed(InsufficientGeneration)".to_string());
    }
    let main_summary = main_summary.ok_or_else(|| "main grid summary is missing".to_string())?;
    let control_summary =
        control_summary.ok_or_else(|| "control grid summary is missing".to_string())?;
    let expected_generation = layout.generator_count() as f32;
    let expected_demand = repeated_lamp_demand(layout.supplied_lamps.len());
    if main_summary.mode != PowerAllocationMode::PriorityPrefix
        || !close(main_grid.generation, expected_generation)
        || !close(main_grid.consumption, expected_demand)
        || !main_grid.powered
        || main_summary.consumer_count != supplied_lamps.len()
        || main_summary.supplied_count != supplied_lamps.len()
        || main_summary.shed_count != 0
    {
        return Err(format!(
            "main grid allocation summary differs from {expected_generation}/{expected_demand}"
        ));
    }
    if control_summary.mode != PowerAllocationMode::PriorityPrefix
        || !close(control_grid.generation, 0.0)
        || !close(control_grid.consumption, 0.2)
        || control_grid.powered
        || control_summary.supplied_count != 0
        || control_summary.shed_count != 1
    {
        return Err(
            "control grid allocation summary differs from the negative control".to_string(),
        );
    }

    for (spa_ordinal, (spa_spec, spa_entities)) in layout.spas.iter().zip(&fixture.spas).enumerate()
    {
        let (site, generator, generates_for) = p
            .q_spas
            .get(spa_entities.site)
            .map_err(|_| format!("SoulSpa {spa_ordinal} production components are missing"))?;
        if site.phase != SoulSpaPhase::Operational
            || site.bones_delivered != site.bones_required
            || site.active_slots != SOUL_SPA_MAX_ACTIVE_SLOTS
            || !close(generator.current_output, spa_spec.workers.len() as f32)
            || generates_for.map(|relation| relation.0) != Some(main_grid_entity)
        {
            return Err(format!(
                "SoulSpa {spa_ordinal} operational generator topology differs from the contract"
            ));
        }
        for (tile_ordinal, (&tile_entity, &expected_grid)) in spa_entities
            .tiles
            .iter()
            .zip(spa_spec.tiles.iter())
            .enumerate()
        {
            let (tile, designation, slots, workers) = p
                .q_tiles
                .get(tile_entity)
                .map_err(|_| format!("SoulSpa {spa_ordinal} tile {tile_ordinal} is missing"))?;
            let expected_worker = spa_spec
                .workers
                .iter()
                .find(|worker| worker.tile_ordinal == tile_ordinal)
                .map(|worker| fixture.soul_entities[worker.soul_ordinal]);
            let workers_are_exact = match expected_worker {
                Some(worker) => workers.is_some_and(|workers| {
                    workers.len() == 1 && workers.iter().any(|entity| *entity == worker)
                }),
                None => workers.is_none_or(TaskWorkers::is_empty),
            };
            if tile.parent_site != spa_entities.site
                || tile.grid_pos != expected_grid
                || designation.map(|value| value.work_type) != Some(WorkType::GeneratePower)
                || slots.map(|value| value.max) != Some(1)
                || !workers_are_exact
            {
                return Err(format!(
                    "SoulSpa {spa_ordinal} tile {tile_ordinal} state differs from the contract"
                ));
            }
        }
    }

    let generator_assignments = layout
        .spas
        .iter()
        .enumerate()
        .flat_map(|(spa_ordinal, spa)| {
            spa.workers
                .iter()
                .map(move |worker| (worker.soul_ordinal, (spa_ordinal, worker.tile_ordinal)))
        })
        .collect::<BTreeMap<_, _>>();
    for ((&entity, &expected_grid), ordinal) in fixture
        .soul_entities
        .iter()
        .zip(&fixture.layout.soul_cells)
        .zip(0..)
    {
        let (transform, task, working_on, identity, soul) = p
            .q_souls
            .get(entity)
            .map_err(|_| format!("Soul ordinal {ordinal} is missing"))?;
        if WorldMap::world_to_grid(transform.translation.truncate()) != expected_grid {
            return Err(format!(
                "Soul ordinal {ordinal} moved before the initial checkpoint"
            ));
        }
        if let Some(&(spa_ordinal, tile_ordinal)) = generator_assignments.get(&ordinal) {
            let tile = fixture.spas[spa_ordinal].tiles[tile_ordinal];
            let task_is_generating = matches!(
                task,
                AssignedTask::GeneratePower(data)
                    if data.tile == tile && data.phase == GeneratePowerPhase::Generating
            );
            if !task_is_generating
                || working_on.map(|value| value.0) != Some(tile)
                || !identity.is_some_and(|value| value.matches_working_on(Some(tile)))
                || soul.dream < 1.0
            {
                return Err("generator Soul task identity is not durable and active".to_string());
            }
        } else if !matches!(task, AssignedTask::None) || working_on.is_some() || identity.is_some()
        {
            return Err(format!(
                "non-generator Soul ordinal {ordinal} has an active task"
            ));
        }
    }
    for ((&entity, &expected_grid), ordinal) in fixture
        .familiar_entities
        .iter()
        .zip(&fixture.layout.familiar_cells)
        .zip(0..)
    {
        let (transform, command, operation) = p
            .q_familiars
            .get(entity)
            .map_err(|_| format!("Familiar ordinal {ordinal} is missing"))?;
        if WorldMap::world_to_grid(transform.translation.truncate()) != expected_grid
            || command.command != FamiliarCommand::Idle
            || operation.max_controlled_soul != 0
        {
            return Err(format!(
                "Familiar ordinal {ordinal} differs from the static contract"
            ));
        }
    }

    let room_entities = validate_rooms(layout, &p.q_rooms, &p.room_tiles, &p.room_boundaries)?;
    validate_showcase_components(fixture, &dedicated_showcase, p)?;

    let mut expected_by_kind = Vec::<(BuildingType, Grid, &ObservedBuilding)>::new();
    expected_by_kind.extend(
        layout
            .walls
            .iter()
            .copied()
            .zip(walls.iter().copied())
            .map(|(grid, building)| (BuildingType::Wall, grid, building)),
    );
    expected_by_kind.extend(
        layout
            .doors
            .iter()
            .zip(doors.iter().copied())
            .map(|(spec, building)| (BuildingType::Door, spec.grid, building)),
    );
    expected_by_kind.extend(
        layout
            .floors
            .iter()
            .copied()
            .zip(floors.iter().copied())
            .map(|(grid, building)| (BuildingType::Floor, grid, building)),
    );
    for (entry, building) in &dedicated_showcase {
        expected_by_kind.push((entry.kind, entry.anchor, building));
    }
    expected_by_kind.extend(
        layout
            .spas
            .iter()
            .zip(soul_spas.iter().copied())
            .map(|(spec, building)| (BuildingType::SoulSpa, spec.anchor, building)),
    );
    expected_by_kind.extend(
        layout
            .supplied_lamps
            .iter()
            .copied()
            .zip(supplied_lamps.iter().copied())
            .map(|(grid, building)| (BuildingType::OutdoorLamp, grid, building)),
    );
    expected_by_kind.push((BuildingType::OutdoorLamp, layout.control_lamp, control_lamp));

    let presentation_order = if layout.showcase.is_empty() {
        vec![
            BuildingType::Floor,
            BuildingType::Wall,
            BuildingType::Door,
            BuildingType::OutdoorLamp,
            BuildingType::SoulSpa,
        ]
    } else {
        BuildingType::ALL.to_vec()
    };
    let presentation = presentation_order
        .iter()
        .filter_map(|&kind| {
            let entries = expected_by_kind
                .iter()
                .filter(|(entry_kind, ..)| *entry_kind == kind)
                .map(|(_, _, building)| *building)
                .collect::<Vec<_>>();
            (!entries.is_empty()).then(|| observe_presentation(building_type_name(kind), &entries))
        })
        .collect();

    let mut audit_buildings = Vec::with_capacity(expected_by_kind.len());
    for kind in BuildingType::ALL {
        for (ordinal, (_, anchor, building)) in expected_by_kind
            .iter()
            .filter(|(entry_kind, ..)| *entry_kind == kind)
            .enumerate()
        {
            let door_state = (kind == BuildingType::Door)
                .then(|| {
                    layout
                        .doors
                        .iter()
                        .find(|spec| spec.grid == *anchor)
                        .map(|spec| spec.state)
                })
                .flatten();
            let lamp_role =
                (kind == BuildingType::OutdoorLamp).then_some(if *anchor == layout.control_lamp {
                    LampRole::Control
                } else {
                    LampRole::Main
                });
            audit_buildings.push(IndoorLightAuditBuilding {
                entity: building.entity,
                kind,
                ordinal: ordinal as u32,
                anchor: *anchor,
                draw_pos: building.draw_pos,
                door_state,
                lamp_role,
            });
        }
    }
    let audit_entities = IndoorLightAuditEntities {
        buildings: audit_buildings,
        yards: [fixture.main_yard, fixture.control_yard],
        grids: [main_grid_entity, control_grid_entity],
        spas: fixture
            .spas
            .iter()
            .map(|spa| IndoorLightAuditSpa {
                site: spa.site,
                tiles: spa.tiles.clone(),
            })
            .collect(),
    };

    let observation = IndoorLightFixtureObservation {
        case_id: format!(
            "indoor-light-{}-{}-seed-{}",
            layout.size.as_str(),
            p.config.render_mode.as_str(),
            p.config.master_seed
        ),
        layout_checksum: layout.layout_checksum,
        floors: floors.len(),
        walls: walls.len(),
        doors: doors.len(),
        rooms: room_entities.len(),
        room_tiles: layout.floors.len(),
        room_boundaries: layout.room_boundaries().len(),
        souls: fixture.soul_entities.len(),
        familiars: fixture.familiar_entities.len(),
        soul_spas: fixture.spas.len(),
        generator_souls: layout.generator_count(),
        yards: 2,
        main_generation: main_grid.generation,
        main_demand: main_grid.consumption,
        main_headroom: main_grid.generation - main_grid.consumption,
        main_supplied_count: main_summary.supplied_count,
        main_shed_count: main_summary.shed_count,
        control_generation: control_grid.generation,
        control_demand: control_grid.consumption,
        control_supplied_count: control_summary.supplied_count,
        control_shed_count: control_summary.shed_count,
        presentation,
    };
    Ok((observation, audit_entities))
}

fn expected_presentation(kind: BuildingType) -> (usize, usize) {
    match kind {
        BuildingType::Floor | BuildingType::Wall => (0, 1),
        BuildingType::Bridge => (1, 0),
        _ => (1, 1),
    }
}

fn building_type_name(kind: BuildingType) -> &'static str {
    match kind {
        BuildingType::Wall => "Wall",
        BuildingType::Door => "Door",
        BuildingType::Floor => "Floor",
        BuildingType::Tank => "Tank",
        BuildingType::MudMixer => "MudMixer",
        BuildingType::RestArea => "RestArea",
        BuildingType::Bridge => "Bridge",
        BuildingType::SandPile => "SandPile",
        BuildingType::BonePile => "BonePile",
        BuildingType::WheelbarrowParking => "WheelbarrowParking",
        BuildingType::SoulSpa => "SoulSpa",
        BuildingType::OutdoorLamp => "OutdoorLamp",
    }
}

fn repeated_lamp_demand(count: usize) -> f32 {
    (0..count).fold(0.0, |sum, _| sum + hw_energy::OUTDOOR_LAMP_DEMAND)
}

fn validate_rooms(
    layout: &IndoorLightLayout,
    q_rooms: &Query<(Entity, &Room)>,
    room_tiles: &RoomTileLookup,
    room_boundaries: &RoomBoundaryLookup,
) -> Result<Vec<Entity>, String> {
    let mut room_entities = Vec::with_capacity(layout.module_count * layout.module_count);
    let wall_cells = layout.walls.iter().copied().collect::<HashSet<_>>();
    let door_cells = layout
        .doors
        .iter()
        .map(|door| door.grid)
        .collect::<HashSet<_>>();
    for (room_ordinal, floor_chunk) in layout.floors.chunks_exact(ROOM_INTERIOR_TILES).enumerate() {
        let floor_set = floor_chunk.iter().copied().collect::<HashSet<_>>();
        let matches = q_rooms
            .iter()
            .filter(|(_, room)| room.tiles.iter().copied().collect::<HashSet<_>>() == floor_set)
            .collect::<Vec<_>>();
        let [(room_entity, room)] = matches.as_slice() else {
            return Err(format!(
                "expected exactly one fixture Room {room_ordinal}, observed {}",
                matches.len()
            ));
        };
        let room_entity = *room_entity;
        let expected_walls = wall_cells
            .iter()
            .copied()
            .filter(|grid| adjacent_to_any(*grid, &floor_set))
            .collect::<HashSet<_>>();
        let expected_doors = door_cells
            .iter()
            .copied()
            .filter(|grid| adjacent_to_any(*grid, &floor_set))
            .collect::<HashSet<_>>();
        if room.wall_tiles.iter().copied().collect::<HashSet<_>>() != expected_walls
            || room.door_tiles.iter().copied().collect::<HashSet<_>>() != expected_doors
            || room.tile_count != ROOM_INTERIOR_TILES
            || !floor_set
                .iter()
                .all(|grid| room_tiles.tile_to_room.get(grid).copied() == Some(room_entity))
        {
            return Err(format!(
                "Room {room_ordinal} differs from the 36-tile contract"
            ));
        }
        room_entities.push(room_entity);
    }
    if q_rooms.iter().count() != room_entities.len()
        || room_tiles.tile_to_room.len() != layout.floors.len()
    {
        return Err("Room count or RoomTileLookup differs from the fixture contract".to_string());
    }
    let expected_boundaries = layout.room_boundaries();
    if room_boundaries.boundary_to_rooms.len() != expected_boundaries.len() {
        return Err("RoomBoundaryLookup cell count differs from the fixture contract".to_string());
    }
    for boundary in expected_boundaries {
        let mut observed = room_boundaries.rooms_at(boundary.grid).to_vec();
        observed.sort_unstable_by_key(|entity| entity.to_bits());
        let mut expected = boundary
            .rooms
            .iter()
            .map(|&ordinal| room_entities[ordinal])
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(|entity| entity.to_bits());
        if observed != expected {
            return Err(format!(
                "RoomBoundaryLookup at {:?} differs from adjacent rooms {:?}",
                boundary.grid, boundary.rooms
            ));
        }
    }
    Ok(room_entities)
}

fn adjacent_to_any(grid: Grid, cells: &HashSet<Grid>) -> bool {
    [(0, 1), (1, 0), (0, -1), (-1, 0)]
        .into_iter()
        .any(|(dx, dy)| cells.contains(&(grid.0 + dx, grid.1 + dy)))
}

fn validate_showcase_components(
    fixture: &IndoorLightFixtureEntities,
    dedicated: &[(&ShowcaseSpec, &ObservedBuilding)],
    p: &IndoorLightValidationParams,
) -> Result<(), String> {
    if dedicated.is_empty() {
        return Ok(());
    }
    let building = |kind| {
        dedicated
            .iter()
            .find(|(entry, _)| entry.kind == kind)
            .map(|(_, building)| *building)
            .ok_or_else(|| format!("showcase {kind:?} is missing"))
    };
    let tank = building(BuildingType::Tank)?;
    let tank_stockpile = p
        .q_stockpiles
        .get(tank.entity)
        .map_err(|_| "showcase Tank Stockpile is missing".to_string())?;
    if tank_stockpile.capacity != 50 || tank_stockpile.resource_type != Some(ResourceType::Water) {
        return Err("showcase Tank Stockpile differs from Water/50".to_string());
    }
    let companion = fixture
        .layout
        .showcase
        .iter()
        .find_map(|entry| entry.companion.as_ref())
        .ok_or_else(|| "showcase Tank companion contract is missing".to_string())?;
    if fixture.bucket_storages.len() != companion.occupied_grids.len() {
        return Err("showcase Tank must own two one-tile BucketStorage entities".to_string());
    }
    let mut storage_entities = Vec::new();
    for (&storage_entity, &expected_grid) in fixture
        .bucket_storages
        .iter()
        .zip(&companion.occupied_grids)
    {
        let (entity, stockpile, belongs_to, pending, transform) = p
            .q_bucket_storages
            .get(storage_entity)
            .map_err(|_| "showcase BucketStorage entity is missing".to_string())?;
        if entity != storage_entity
            || stockpile.capacity != 10
            || stockpile.resource_type.is_some()
            || belongs_to.0 != tank.entity
            || pending.is_some()
            || WorldMap::world_to_grid(transform.translation.truncate()) != expected_grid
            || p.world_map.stockpile_entity(expected_grid) != Some(storage_entity)
        {
            return Err(format!(
                "showcase BucketStorage at {expected_grid:?} differs from production topology"
            ));
        }
        storage_entities.push(storage_entity);
    }
    let mut bucket_counts = BTreeMap::<Entity, usize>::new();
    let buckets = p
        .q_owned_items
        .iter()
        .filter(|(_, item, owner, _, _, _, wheelbarrow)| {
            owner.0 == tank.entity && item.0 == ResourceType::BucketEmpty && !*wheelbarrow
        })
        .collect::<Vec<_>>();
    if buckets.len() != 5 {
        return Err(format!(
            "showcase Tank owns {} empty buckets, expected 5",
            buckets.len()
        ));
    }
    for (_, _, _, stored_in, slots, transform, _) in buckets {
        let storage = stored_in
            .map(|relation| relation.0)
            .filter(|entity| storage_entities.contains(entity))
            .ok_or_else(|| "showcase bucket StoredIn relation is invalid".to_string())?;
        let (_, _, _, _, storage_transform) = p
            .q_bucket_storages
            .get(storage)
            .map_err(|_| "showcase bucket storage vanished".to_string())?;
        if slots.max != 1
            || transform.translation.truncate() != storage_transform.translation.truncate()
        {
            return Err("showcase bucket TaskSlots or position differs from storage".to_string());
        }
        *bucket_counts.entry(storage).or_default() += 1;
    }
    if storage_entities
        .iter()
        .map(|entity| bucket_counts.get(entity).copied().unwrap_or_default())
        .collect::<Vec<_>>()
        != [3, 2]
    {
        return Err("showcase Tank bucket distribution differs from deterministic 3/2".to_string());
    }

    let mud_mixer = building(BuildingType::MudMixer)?;
    if !p.q_mud_mixers.contains(mud_mixer.entity)
        || !p
            .q_stockpiles
            .get(mud_mixer.entity)
            .is_ok_and(|stockpile| stockpile.resource_type == Some(ResourceType::Water))
    {
        return Err("showcase MudMixer production components are missing".to_string());
    }
    for (kind, present) in [
        (
            BuildingType::RestArea,
            p.q_rest_areas
                .contains(building(BuildingType::RestArea)?.entity),
        ),
        (
            BuildingType::Bridge,
            p.q_bridges.contains(building(BuildingType::Bridge)?.entity),
        ),
        (
            BuildingType::SandPile,
            p.q_sand_piles
                .contains(building(BuildingType::SandPile)?.entity),
        ),
        (
            BuildingType::BonePile,
            p.q_bone_piles
                .contains(building(BuildingType::BonePile)?.entity),
        ),
    ] {
        if !present {
            return Err(format!("showcase {kind:?} production marker is missing"));
        }
    }
    let parking = building(BuildingType::WheelbarrowParking)?;
    if !p
        .q_wheelbarrow_parking
        .get(parking.entity)
        .is_ok_and(|parking| parking.capacity == 2)
    {
        return Err("showcase WheelbarrowParking capacity differs from 2".to_string());
    }
    let wheelbarrows = p
        .q_owned_items
        .iter()
        .filter(|(_, item, owner, _, _, _, wheelbarrow)| {
            owner.0 == parking.entity && item.0 == ResourceType::Wheelbarrow && *wheelbarrow
        })
        .count();
    if wheelbarrows != 2 {
        return Err(format!(
            "showcase WheelbarrowParking owns {wheelbarrows} wheelbarrows, expected 2"
        ));
    }
    Ok(())
}

fn observe_presentation(
    building_kind: &'static str,
    buildings: &[&ObservedBuilding],
) -> IndoorLightPresentationObservation {
    IndoorLightPresentationObservation {
        building_kind,
        entity_count: buildings.len(),
        root_sprite_count: buildings
            .iter()
            .filter(|building| building.root_sprite)
            .count(),
        child_sprite_count: buildings
            .iter()
            .map(|building| building.child_sprites)
            .sum(),
        owner_3d_count: buildings
            .iter()
            .map(|building| building.owner_visuals)
            .sum(),
    }
}

pub(super) fn collect_indoor_light_audit_records(
    q: &IndoorLightAuditQueries<'_, '_>,
) -> Result<Vec<PerfAuditActorRecord>, String> {
    if q.state.phase == IndoorLightFixturePhase::Inactive {
        return Ok(Vec::new());
    }
    if q.state.phase != IndoorLightFixturePhase::Ready {
        return Err(format!(
            "indoor-light audit requested while fixture is {:?}",
            q.state.phase
        ));
    }
    let tracked = q.state.audit_entities.as_ref().ok_or_else(|| {
        "indoor-light fixture is Ready without tracked audit entities".to_string()
    })?;
    let fixture = q
        .state
        .fixture
        .as_ref()
        .ok_or_else(|| "indoor-light fixture is Ready without fixture entities".to_string())?;
    let mut records = Vec::new();

    for expected in &tracked.buildings {
        let (
            entity,
            building,
            transform,
            door,
            consumer,
            supply,
            consumes_from,
            unpowered,
            children,
        ) = q.q_buildings.get(expected.entity).map_err(|_| {
            format!(
                "tracked indoor-light {:?} {} is missing",
                expected.kind, expected.ordinal
            )
        })?;
        let observed_pos = transform.translation.truncate();
        let child_sprites = children.map_or(0, |children| {
            children
                .iter()
                .filter(|child| q.q_sprites.contains(*child))
                .count()
        });
        let owner_visuals = q
            .q_visuals
            .iter()
            .filter(|visual| visual.owner == entity)
            .count();
        let (expected_child_sprites, expected_owner_visuals) = expected_presentation(expected.kind);
        if building.kind != expected.kind
            || building.is_provisional
            || observed_pos != expected.draw_pos
            || q.q_sprites.contains(entity)
            || child_sprites != expected_child_sprites
            || owner_visuals != expected_owner_visuals
        {
            return Err(format!(
                "tracked indoor-light {:?} {} changed topology or presentation",
                expected.kind, expected.ordinal
            ));
        }

        match expected.kind {
            BuildingType::Door => {
                let state = expected
                    .door_state
                    .ok_or_else(|| "tracked Door is missing expected state".to_string())?;
                let expected_image = if state == hw_core::world::DoorState::Open {
                    &q.door_handles.door_open
                } else {
                    &q.door_handles.door_closed
                };
                let child_images = children.map_or_else(Vec::new, |children| {
                    children
                        .iter()
                        .filter_map(|child| q.q_sprites.get(child).ok())
                        .map(|sprite| sprite.image.clone())
                        .collect::<Vec<_>>()
                });
                if door.map(|door| door.state) != Some(state)
                    || child_images != [expected_image.clone()]
                    || q.world_map
                        .door_entity(expected.anchor.0, expected.anchor.1)
                        != Some(entity)
                    || q.world_map.door_state(expected.anchor.0, expected.anchor.1) != Some(state)
                {
                    return Err(format!(
                        "indoor-light Door {} changed from {:?}",
                        expected.ordinal, state
                    ));
                }
            }
            BuildingType::OutdoorLamp => {
                if !consumer
                    .is_some_and(|consumer| close(consumer.demand, hw_energy::OUTDOOR_LAMP_DEMAND))
                {
                    return Err(format!(
                        "indoor-light Lamp {} changed demand or position",
                        expected.ordinal
                    ));
                }
                let (expected_grid_entity, expected_supply, expected_unpowered) = match expected
                    .lamp_role
                {
                    Some(LampRole::Main) => (tracked.grids[0], PowerSupplyState::Supplied, false),
                    Some(LampRole::Control) => (
                        tracked.grids[1],
                        PowerSupplyState::Shed {
                            reason: PowerShedReason::InsufficientGeneration,
                        },
                        true,
                    ),
                    None => return Err("tracked Lamp is missing its role".to_string()),
                };
                if supply.copied() != Some(expected_supply)
                    || consumes_from.map(|relation| relation.0) != Some(expected_grid_entity)
                    || unpowered != expected_unpowered
                {
                    return Err(format!(
                        "indoor-light Lamp {} changed production supply topology",
                        expected.ordinal
                    ));
                };
            }
            _ => {
                if door.is_some()
                    || consumer.is_some()
                    || supply.is_some()
                    || consumes_from.is_some()
                    || unpowered
                {
                    return Err(format!(
                        "tracked indoor-light {:?} gained Door or consumer state",
                        expected.kind
                    ));
                }
            }
        }

        let mut record = vec![b'B'];
        write_building_type(&mut record, expected.kind);
        write_u64(&mut record, u64::from(expected.ordinal));
        write_transform(&mut record, transform, "indoor-light building transform")?;
        record.push(u8::from(building.is_provisional));
        match door {
            Some(door) => {
                record.push(1);
                write_door_state(&mut record, door.state);
            }
            None => record.push(0),
        }
        match consumer {
            Some(consumer) => {
                record.push(1);
                write_f32(&mut record, consumer.demand, "indoor-light consumer demand")?;
            }
            None => record.push(0),
        }
        write_supply_state(&mut record, supply);
        record.push(power_relation_tag(
            consumes_from.map(|relation| relation.0),
            tracked.grids,
        )?);
        record.push(u8::from(unpowered));
        record.push(u8::from(q.q_sprites.contains(entity)));
        write_u64(&mut record, child_sprites as u64);
        write_u64(&mut record, owner_visuals as u64);
        records.push(PerfAuditActorRecord {
            actor_kind: "indoor-building",
            actor_key: (u64::from(building_type_tag(expected.kind)) << 32)
                | u64::from(expected.ordinal),
            record,
        });
    }

    for (ordinal, yard_entity) in tracked.yards.into_iter().enumerate() {
        let yard = q
            .q_yards
            .get(yard_entity)
            .map_err(|_| format!("indoor-light Yard {ordinal} is missing"))?;
        let expected = if ordinal == 0 {
            fixture.layout.main_yard()
        } else {
            fixture.layout.control_yard()
        };
        if yard.min != expected.min || yard.max != expected.max {
            return Err(format!("indoor-light Yard {ordinal} changed bounds"));
        }
        let mut record = vec![b'Y'];
        write_vec2(&mut record, yard.min, "indoor-light Yard min")?;
        write_vec2(&mut record, yard.max, "indoor-light Yard max")?;
        records.push(PerfAuditActorRecord {
            actor_kind: "indoor-yard",
            actor_key: ordinal as u64,
            record,
        });
    }

    let supplied_lamps = tracked
        .buildings
        .iter()
        .filter(|building| building.lamp_role == Some(LampRole::Main))
        .map(|building| building.entity)
        .collect::<HashSet<_>>();
    let control_lamp = tracked
        .buildings
        .iter()
        .find(|building| building.lamp_role == Some(LampRole::Control))
        .map(|building| building.entity)
        .ok_or_else(|| "indoor-light control Lamp tracking is missing".to_string())?;
    for (ordinal, grid_entity) in tracked.grids.into_iter().enumerate() {
        let (_, grid, owner, summary, generators, consumers) = q
            .q_grids
            .get(grid_entity)
            .map_err(|_| format!("indoor-light PowerGrid {ordinal} is missing"))?;
        let summary = summary.ok_or_else(|| {
            format!("indoor-light PowerGrid {ordinal} allocation summary is missing")
        })?;
        let expected_owner = tracked.yards[ordinal];
        let expected_generation = if ordinal == 0 {
            fixture.layout.generator_count() as f32
        } else {
            0.0
        };
        let expected_demand = if ordinal == 0 {
            repeated_lamp_demand(fixture.layout.supplied_lamps.len())
        } else {
            hw_energy::OUTDOOR_LAMP_DEMAND
        };
        let expected_powered = ordinal == 0;
        let expected_supplied = if ordinal == 0 {
            supplied_lamps.len()
        } else {
            0
        };
        let expected_shed = usize::from(ordinal == 1);
        let generators_are_exact = if ordinal == 0 {
            generators.is_some_and(|items| {
                items.iter().copied().collect::<HashSet<_>>()
                    == tracked
                        .spas
                        .iter()
                        .map(|spa| spa.site)
                        .collect::<HashSet<_>>()
            })
        } else {
            generators.is_none_or(GridGenerators::is_empty)
        };
        let consumers_are_exact = if ordinal == 0 {
            consumers.is_some_and(|items| {
                items.iter().copied().collect::<HashSet<_>>() == supplied_lamps
            })
        } else {
            consumers.is_some_and(|items| {
                items.len() == 1 && items.iter().any(|entity| *entity == control_lamp)
            })
        };
        let shed_order_is_exact = if ordinal == 0 {
            summary.shed_order.is_empty()
        } else {
            summary.shed_order == [control_lamp]
        };
        if owner.0 != expected_owner
            || !close(grid.generation, expected_generation)
            || !close(grid.consumption, expected_demand)
            || grid.powered != expected_powered
            || summary.mode != PowerAllocationMode::PriorityPrefix
            || !close(summary.generation, expected_generation)
            || !close(summary.total_demand, expected_demand)
            || !close(
                summary.served_demand,
                if ordinal == 0 { expected_demand } else { 0.0 },
            )
            || summary.consumer_count
                != if ordinal == 0 {
                    supplied_lamps.len()
                } else {
                    1
                }
            || summary.supplied_count != expected_supplied
            || summary.shed_count != expected_shed
            || summary.invalid_count != 0
            || !shed_order_is_exact
            || !generators_are_exact
            || !consumers_are_exact
        {
            return Err(format!(
                "indoor-light PowerGrid {ordinal} drifted from the production topology contract"
            ));
        }

        let mut record = vec![b'G'];
        write_f32(&mut record, grid.generation, "indoor-light grid generation")?;
        write_f32(&mut record, grid.consumption, "indoor-light grid demand")?;
        record.push(u8::from(grid.powered));
        record.push(ordinal as u8);
        record.push(match summary.mode {
            PowerAllocationMode::LegacyAllOrNone => 0,
            PowerAllocationMode::PriorityPrefix => 1,
        });
        write_f32(
            &mut record,
            summary.generation,
            "indoor-light summary generation",
        )?;
        write_f32(
            &mut record,
            summary.total_demand,
            "indoor-light summary total demand",
        )?;
        write_f32(
            &mut record,
            summary.served_demand,
            "indoor-light summary served demand",
        )?;
        for value in [
            summary.consumer_count,
            summary.supplied_count,
            summary.shed_count,
            summary.invalid_count,
            summary.shed_order.len(),
            generators.map_or(0, GridGenerators::len),
            consumers.map_or(0, GridConsumers::len),
        ] {
            write_u64(&mut record, value as u64);
        }
        records.push(PerfAuditActorRecord {
            actor_kind: "indoor-grid",
            actor_key: ordinal as u64,
            record,
        });
    }

    for (spa_ordinal, (tracked_spa, spa_spec)) in
        tracked.spas.iter().zip(&fixture.layout.spas).enumerate()
    {
        let (site, generator, generates_for) = q
            .q_spas
            .get(tracked_spa.site)
            .map_err(|_| format!("tracked indoor-light SoulSpa {spa_ordinal} is missing"))?;
        if site.phase != SoulSpaPhase::Operational
            || site.bones_delivered != site.bones_required
            || site.active_slots != SOUL_SPA_MAX_ACTIVE_SLOTS
            || !close(generator.current_output, spa_spec.workers.len() as f32)
            || generates_for.map(|relation| relation.0) != Some(tracked.grids[0])
        {
            return Err(format!(
                "indoor-light SoulSpa {spa_ordinal} drifted from Operational output"
            ));
        }
        let mut spa_record = vec![b'S', b'P'];
        spa_record.push(match site.phase {
            SoulSpaPhase::Constructing => 0,
            SoulSpaPhase::Operational => 1,
        });
        for value in [site.bones_required, site.bones_delivered, site.active_slots] {
            spa_record.extend_from_slice(&value.to_le_bytes());
        }
        write_f32(
            &mut spa_record,
            generator.current_output,
            "indoor-light SoulSpa output",
        )?;
        write_f32(
            &mut spa_record,
            generator.output_per_soul,
            "indoor-light SoulSpa output per Soul",
        )?;
        spa_record.push(power_relation_tag(
            generates_for.map(|relation| relation.0),
            tracked.grids,
        )?);
        records.push(PerfAuditActorRecord {
            actor_kind: "indoor-soul-spa",
            actor_key: spa_ordinal as u64,
            record: spa_record,
        });

        for (tile_ordinal, (&tile_entity, &expected_grid)) in tracked_spa
            .tiles
            .iter()
            .zip(spa_spec.tiles.iter())
            .enumerate()
        {
            let (tile, designation, slots, workers) = q.q_tiles.get(tile_entity).map_err(|_| {
                format!("tracked indoor-light SoulSpa {spa_ordinal} tile {tile_ordinal} is missing")
            })?;
            let expected_worker = spa_spec
                .workers
                .iter()
                .find(|worker| worker.tile_ordinal == tile_ordinal)
                .map(|worker| fixture.soul_entities[worker.soul_ordinal]);
            let worker_is_exact = match expected_worker {
                Some(worker) => workers.is_some_and(|workers| {
                    workers.len() == 1 && workers.iter().any(|entity| *entity == worker)
                }),
                None => workers.is_none_or(TaskWorkers::is_empty),
            };
            if tile.parent_site != tracked_spa.site
                || tile.grid_pos != expected_grid
                || designation.map(|designation| designation.work_type)
                    != Some(WorkType::GeneratePower)
                || slots.map(|slots| slots.max) != Some(1)
                || !worker_is_exact
            {
                return Err(format!(
                    "indoor-light SoulSpa {spa_ordinal} tile {tile_ordinal} drifted from the worker contract"
                ));
            }
            let mut record = vec![b'T'];
            write_grid_pos(&mut record, tile.grid_pos);
            write_work_type(
                &mut record,
                designation.expect("validated designation").work_type,
            );
            write_u64(
                &mut record,
                u64::from(slots.expect("validated TaskSlots").max),
            );
            write_u64(&mut record, workers.map_or(0, TaskWorkers::len) as u64);
            records.push(PerfAuditActorRecord {
                actor_kind: "indoor-soul-spa-tile",
                actor_key: (spa_ordinal * 4 + tile_ordinal) as u64,
                record,
            });
        }
    }

    validate_showcase_audit_components(fixture, tracked, q)?;
    append_indoor_room_audit_records(&mut records, q, fixture)?;
    Ok(records)
}

fn append_indoor_room_audit_records(
    records: &mut Vec<PerfAuditActorRecord>,
    q: &IndoorLightAuditQueries<'_, '_>,
    fixture: &IndoorLightFixtureEntities,
) -> Result<(), String> {
    validate_rooms(
        &fixture.layout,
        &q.q_rooms,
        &q.room_tiles,
        &q.room_boundaries,
    )?;
    for (room_ordinal, floor_chunk) in fixture
        .layout
        .floors
        .chunks_exact(ROOM_INTERIOR_TILES)
        .enumerate()
    {
        let floor_set = floor_chunk.iter().copied().collect::<HashSet<_>>();
        let (_, room) = q
            .q_rooms
            .iter()
            .find(|(_, room)| room.tiles.iter().copied().collect::<HashSet<_>>() == floor_set)
            .ok_or_else(|| format!("indoor-light Room {room_ordinal} vanished"))?;
        let mut room_record = vec![b'R'];
        let mut tiles = room.tiles.clone();
        tiles.sort_unstable();
        let mut walls = room.wall_tiles.clone();
        walls.sort_unstable();
        let mut doors = room.door_tiles.clone();
        doors.sort_unstable();
        for grids in [&tiles, &walls, &doors] {
            write_u64(&mut room_record, grids.len() as u64);
            for &grid in grids {
                write_grid_pos(&mut room_record, grid);
            }
        }
        write_grid_pos(&mut room_record, (room.bounds.min_x, room.bounds.min_y));
        write_grid_pos(&mut room_record, (room.bounds.max_x, room.bounds.max_y));
        write_u64(&mut room_record, room.tile_count as u64);
        records.push(PerfAuditActorRecord {
            actor_kind: "indoor-room",
            actor_key: room_ordinal as u64,
            record: room_record,
        });
    }

    let mut lookup_record = vec![b'L'];
    let mut tile_keys = q
        .room_tiles
        .tile_to_room
        .keys()
        .copied()
        .collect::<Vec<_>>();
    tile_keys.sort_unstable();
    let mut boundary_keys = q
        .room_boundaries
        .boundary_to_rooms
        .keys()
        .copied()
        .collect::<Vec<_>>();
    boundary_keys.sort_unstable();
    for grids in [&tile_keys, &boundary_keys] {
        write_u64(&mut lookup_record, grids.len() as u64);
        for &grid in grids {
            write_grid_pos(&mut lookup_record, grid);
        }
    }
    records.push(PerfAuditActorRecord {
        actor_kind: "indoor-room-lookup",
        actor_key: 0,
        record: lookup_record,
    });
    Ok(())
}

fn validate_showcase_audit_components(
    fixture: &IndoorLightFixtureEntities,
    tracked: &IndoorLightAuditEntities,
    q: &IndoorLightAuditQueries<'_, '_>,
) -> Result<(), String> {
    if fixture.layout.showcase.is_empty() {
        return Ok(());
    }
    let entity = |kind| {
        tracked
            .buildings
            .iter()
            .find(|building| building.kind == kind)
            .map(|building| building.entity)
            .ok_or_else(|| format!("tracked showcase {kind:?} is missing"))
    };
    let tank = entity(BuildingType::Tank)?;
    if !q.q_stockpiles.get(tank).is_ok_and(|stockpile| {
        stockpile.capacity == 50 && stockpile.resource_type == Some(ResourceType::Water)
    }) {
        return Err("tracked showcase Tank Stockpile drifted".to_string());
    }
    for &storage in &fixture.bucket_storages {
        let (_, stockpile, belongs_to, pending, transform) = q
            .q_bucket_storages
            .get(storage)
            .map_err(|_| "tracked showcase BucketStorage vanished".to_string())?;
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        if stockpile.capacity != 10
            || stockpile.resource_type.is_some()
            || belongs_to.0 != tank
            || pending.is_some()
            || q.world_map.stockpile_entity(grid) != Some(storage)
        {
            return Err("tracked showcase BucketStorage drifted".to_string());
        }
    }
    let bucket_count = q
        .q_owned_items
        .iter()
        .filter(|(_, item, owner, stored_in, slots, _, wheelbarrow)| {
            owner.0 == tank
                && item.0 == ResourceType::BucketEmpty
                && !*wheelbarrow
                && slots.max == 1
                && stored_in.is_some_and(|stored| fixture.bucket_storages.contains(&stored.0))
        })
        .count();
    if bucket_count != 5 {
        return Err("tracked showcase Tank bucket topology drifted".to_string());
    }
    let mud_mixer = entity(BuildingType::MudMixer)?;
    if !q.q_mud_mixers.contains(mud_mixer)
        || !q
            .q_stockpiles
            .get(mud_mixer)
            .is_ok_and(|stockpile| stockpile.resource_type == Some(ResourceType::Water))
    {
        return Err("tracked showcase MudMixer drifted".to_string());
    }
    for (kind, present) in [
        (
            BuildingType::RestArea,
            q.q_rest_areas.contains(entity(BuildingType::RestArea)?),
        ),
        (
            BuildingType::Bridge,
            q.q_bridges.contains(entity(BuildingType::Bridge)?),
        ),
        (
            BuildingType::SandPile,
            q.q_sand_piles.contains(entity(BuildingType::SandPile)?),
        ),
        (
            BuildingType::BonePile,
            q.q_bone_piles.contains(entity(BuildingType::BonePile)?),
        ),
    ] {
        if !present {
            return Err(format!("tracked showcase {kind:?} marker drifted"));
        }
    }
    let parking = entity(BuildingType::WheelbarrowParking)?;
    if !q
        .q_wheelbarrow_parking
        .get(parking)
        .is_ok_and(|parking| parking.capacity == 2)
        || q.q_owned_items
            .iter()
            .filter(|(_, item, owner, _, _, _, wheelbarrow)| {
                owner.0 == parking && item.0 == ResourceType::Wheelbarrow && *wheelbarrow
            })
            .count()
            != 2
    {
        return Err("tracked showcase WheelbarrowParking drifted".to_string());
    }
    Ok(())
}

fn write_supply_state(record: &mut Vec<u8>, supply: Option<&PowerSupplyState>) {
    match supply {
        None => record.push(0),
        Some(PowerSupplyState::Supplied) => record.push(1),
        Some(PowerSupplyState::Shed { reason }) => {
            record.push(2);
            record.push(match reason {
                PowerShedReason::InsufficientGeneration => 0,
                PowerShedReason::RestoreMargin => 1,
                PowerShedReason::LegacyGlobalDeficit => 2,
            });
        }
        Some(PowerSupplyState::Disconnected) => record.push(3),
        Some(PowerSupplyState::InvalidDemand) => record.push(4),
    }
}

fn power_relation_tag(relation: Option<Entity>, grids: [Entity; 2]) -> Result<u8, String> {
    match relation {
        None => Ok(0),
        Some(entity) if entity == grids[0] => Ok(1),
        Some(entity) if entity == grids[1] => Ok(2),
        Some(entity) => Err(format!(
            "indoor-light power relation references untracked entity {entity:?}"
        )),
    }
}

fn building_type_tag(kind: BuildingType) -> u8 {
    match kind {
        BuildingType::Wall => 0,
        BuildingType::Door => 1,
        BuildingType::Floor => 2,
        BuildingType::Tank => 3,
        BuildingType::MudMixer => 4,
        BuildingType::RestArea => 5,
        BuildingType::Bridge => 6,
        BuildingType::SandPile => 7,
        BuildingType::BonePile => 8,
        BuildingType::WheelbarrowParking => 9,
        BuildingType::SoulSpa => 10,
        BuildingType::OutdoorLamp => 11,
    }
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() <= f32::EPSILON
}

fn fail_fixture(
    state: &mut IndoorLightFixtureState,
    exit: &mut MessageWriter<AppExit>,
    reason: String,
) {
    error!("PERF_CAPTURE: indoor-light fixture rejected: {reason}");
    state.phase = IndoorLightFixturePhase::Failed;
    state.failure = Some(reason);
    exit.write(AppExit::error());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_sidecar_state(size: PerfScenarioSize) -> IndoorLightFixtureState {
        let layout = IndoorLightLayout::build(size);
        let presentation_order = if layout.showcase.is_empty() {
            vec![
                BuildingType::Floor,
                BuildingType::Wall,
                BuildingType::Door,
                BuildingType::OutdoorLamp,
                BuildingType::SoulSpa,
            ]
        } else {
            BuildingType::ALL.to_vec()
        };
        let presentation = presentation_order
            .into_iter()
            .map(|kind| {
                let canonical_count = match kind {
                    BuildingType::Floor => layout.floors.len(),
                    BuildingType::Wall => layout.walls.len(),
                    BuildingType::Door => layout.doors.len(),
                    BuildingType::SoulSpa => layout.spas.len(),
                    BuildingType::OutdoorLamp => layout.supplied_lamps.len() + 1,
                    _ => 0,
                };
                let dedicated_count = layout
                    .showcase
                    .iter()
                    .filter(|entry| entry.kind == kind && entry.is_dedicated())
                    .count();
                let entity_count = canonical_count + dedicated_count;
                let (child_sprites, owner_visuals) = expected_presentation(kind);
                IndoorLightPresentationObservation {
                    building_kind: building_type_name(kind),
                    entity_count,
                    root_sprite_count: 0,
                    child_sprite_count: entity_count * child_sprites,
                    owner_3d_count: entity_count * owner_visuals,
                }
            })
            .collect();
        let observation = IndoorLightFixtureObservation {
            case_id: format!("sidecar-test-{}", size.as_str()),
            layout_checksum: layout.layout_checksum,
            floors: layout.floors.len(),
            walls: layout.walls.len(),
            doors: layout.doors.len(),
            rooms: layout.module_count * layout.module_count,
            room_tiles: layout.floors.len(),
            room_boundaries: layout.room_boundaries().len(),
            souls: layout.soul_cells.len(),
            familiars: layout.familiar_cells.len(),
            soul_spas: layout.spas.len(),
            generator_souls: layout.generator_count(),
            yards: 2,
            main_generation: layout.generator_count() as f32,
            main_demand: repeated_lamp_demand(layout.supplied_lamps.len()),
            main_headroom: layout.generator_count() as f32
                - repeated_lamp_demand(layout.supplied_lamps.len()),
            main_supplied_count: layout.supplied_lamps.len(),
            main_shed_count: 0,
            control_generation: 0.0,
            control_demand: hw_energy::OUTDOOR_LAMP_DEMAND,
            control_supplied_count: 0,
            control_shed_count: 1,
            presentation,
        };
        IndoorLightFixtureState {
            phase: IndoorLightFixturePhase::Ready,
            fixture: Some(IndoorLightFixtureEntities {
                layout,
                soul_entities: Vec::new(),
                familiar_entities: Vec::new(),
                main_yard: Entity::PLACEHOLDER,
                control_yard: Entity::PLACEHOLDER,
                spas: Vec::new(),
                bucket_storages: Vec::new(),
            }),
            observation: Some(observation),
            ..default()
        }
    }

    #[test]
    fn small_layout_matches_the_pinned_contract_geometry() {
        let layout = IndoorLightLayout::build(PerfScenarioSize::Small);
        assert_eq!(layout.floors.len(), 36);
        assert_eq!(layout.walls.len(), 27);
        assert_eq!(
            layout.doors,
            [DoorSpec {
                grid: (19, 27),
                state: hw_core::world::DoorState::Closed,
            }]
        );
        assert_eq!(layout.supplied_lamps, [(17, 21)]);
        assert_eq!(layout.control_lamp, (80, 80));
        assert_eq!(layout.soul_cells.len(), 50);
        assert_eq!(
            layout.familiar_cells,
            [(17, 24), (18, 24), (19, 24), (20, 24)]
        );
        assert_eq!(layout.soul_cells[28], (21, 25));
        assert_eq!(layout.spas[0].tiles[2], (21, 25));
        assert!(!layout.familiar_cells.contains(&(21, 25)));
        assert_eq!(layout.room_boundaries().len(), 24);
        assert_eq!(SMALL_LAYOUT_SHA256.len(), 64);
    }

    #[test]
    fn medium_and_large_layouts_match_the_exact_matrix() {
        let medium = IndoorLightLayout::build(PerfScenarioSize::Medium);
        assert_eq!(medium.floors.len(), 144);
        assert_eq!(medium.walls.len(), 77);
        assert_eq!(medium.doors.len(), 4);
        assert_eq!(medium.supplied_lamps.len(), 10);
        assert_eq!(medium.soul_cells.len(), 200);
        assert_eq!(medium.familiar_cells.len(), 12);
        assert_eq!(medium.control_lamp, (82, 80));
        assert_eq!(medium.generator_count(), 3);
        assert_eq!(medium.room_boundaries().len(), 72);
        assert_eq!(medium.showcase.len(), BuildingType::ALL.len());
        assert_eq!(
            medium
                .doors
                .iter()
                .map(|door| door.state)
                .collect::<Vec<_>>(),
            [
                hw_core::world::DoorState::Open,
                hw_core::world::DoorState::Open,
                hw_core::world::DoorState::Closed,
                hw_core::world::DoorState::Locked,
            ]
        );

        let large = IndoorLightLayout::build(PerfScenarioSize::Large);
        assert_eq!(large.floors.len(), 576);
        assert_eq!(large.walls.len(), 249);
        assert_eq!(large.doors.len(), 16);
        assert_eq!(large.supplied_lamps.len(), 50);
        assert_eq!(large.soul_cells.len(), 500);
        assert_eq!(large.familiar_cells.len(), 30);
        assert_eq!(large.control_lamp, (84, 80));
        assert_eq!(large.spas.len(), 3);
        assert_eq!(large.generator_count(), 11);
        assert_eq!(large.room_boundaries().len(), 240);
        assert_eq!(large.showcase.len(), BuildingType::ALL.len());
        assert_eq!(
            large
                .showcase
                .iter()
                .find(|entry| entry.kind == BuildingType::Tank)
                .and_then(|entry| entry.companion.as_ref())
                .map(|companion| companion.occupied_grids.as_slice()),
            Some(&[(17, 30), (18, 30)][..])
        );
    }

    #[test]
    fn rust_sidecars_cover_all_runtime_sizes() {
        for (size, expected_layout_rows, expected_presentation_rows) in [
            (PerfScenarioSize::Small, 187, 5),
            (PerfScenarioSize::Medium, 722, 12),
            (PerfScenarioSize::Large, 2306, 12),
        ] {
            let state = ready_sidecar_state(size);
            let (summary, layout, presentation) = state.sidecar_csvs(STAGE_ID, LANE).unwrap();
            assert_eq!(summary.lines().count(), 2);
            assert_eq!(layout.lines().count() - 1, expected_layout_rows);
            assert_eq!(presentation.lines().count() - 1, expected_presentation_rows);
            assert!(summary.contains(CONTRACT_SHA256));
            assert!(summary.contains(FIXTURE_SHA256));

            let rows = layout
                .lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let showcase_count = rows
                .iter()
                .filter(|row| row[1] == "showcase_building")
                .count();
            if size == PerfScenarioSize::Small {
                assert_eq!(showcase_count, 0);
            } else {
                assert_eq!(showcase_count, BuildingType::ALL.len());
                let bridge = rows
                    .iter()
                    .find(|row| row[1] == "showcase_building" && row[7] == "Bridge")
                    .unwrap();
                assert_eq!((bridge[3], bridge[4]), ("90", "65"));
                assert_eq!(
                    rows.iter()
                        .filter(|row| row[1] == "showcase_companion")
                        .count(),
                    1
                );
                assert_eq!(
                    rows.iter()
                        .filter(|row| row[1] == "showcase_companion_footprint")
                        .count(),
                    2
                );
            }
        }
    }
}
