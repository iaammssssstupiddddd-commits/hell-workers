//! Same 16×16 rooms, with a different number of rooms sharing a building envelope.
use super::{
    PerfCapture, PerfCapturePhase, PerfScenarioConfig, PerfScenarioSize,
    fixture::PerfMainCameraQuery,
};
use crate::{
    assets::GameAssets, plugins::startup::Building3dHandles, systems::ventilation::perf::AirPerf,
};
use bevy::prelude::*;
use hw_core::world::DoorState;
use hw_jobs::{Building, BuildingType, Door};
use hw_world::{WorldMap, WorldMapWrite};
use serde::Serialize;

const ROOMS_PER_SIDE: i32 = 16;
const ROOM_PITCH: i32 = 3;
const SECONDS_PER_TRANSITION: f64 = 0.5;

#[derive(Resource)]
pub(crate) struct Fixture {
    doors: Vec<(Entity, (i32, i32))>,
    side: i32,
    adjustable_fan: Option<Entity>,
    last_climate_tick: u64,
    generator: Option<Entity>,
    transitions: usize,
    samples: Vec<Sample>,
    measure_builds: Option<u64>,
}

#[derive(Serialize)]
struct Sample {
    phase: &'static str,
    real_delta_ms: f64,
    cpu_ns: u64,
    climate_cpu_ns: u64,
    climate_tick: u64,
    climate_zones: usize,
    process_cpu_ns: u64,
    prepare_ns: u64,
    step_ns: u64,
    publish_ns: u64,
    powered_fans: usize,
    high_fans: usize,
    input_ns: u64,
    wind: f64,
    inspected_net: f64,
    rooms_written: usize,
    vents_written: usize,
    groups_changed: usize,
    publish_allocations: u64,
    publish_bytes: u64,
    inspection_ns: u64,
    inspections: u64,
    exterior_net_abs: f64,
    transitions: usize,
    speed: f64,
    calls: u64,
    rebuilds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<crate::systems::ventilation::transport_probe::Observation>,
}

pub(super) fn spawn(
    commands: &mut Commands,
    config: &PerfScenarioConfig,
    map: &mut WorldMap,
    assets: &GameAssets,
    handles: &Building3dHandles,
    cameras: &mut PerfMainCameraQuery,
) {
    let side = match config.size() {
        PerfScenarioSize::Small => 2,
        PerfScenarioSize::Medium => 4,
        PerfScenarioSize::Large => 16,
    };
    let blocks = ROOMS_PER_SIDE / side;
    let span = side * ROOM_PITCH;
    let pitch = span + 2;
    let origin = (100 - (blocks * pitch - 1)) / 2;
    let mut doors = Vec::new();
    let schedule = std::env::var("HW_AIR_OUTAGE").unwrap_or_default();
    let mixed = schedule == "mixed";
    let mut adjustable_fan = None;
    let mut pinned_vent = None;
    for by in 0..blocks {
        for bx in 0..blocks {
            for y in 0..=span {
                for x in 0..=span {
                    let vertical = x % ROOM_PITCH == 0;
                    let horizontal = y % ROOM_PITCH == 0;
                    let kind = if horizontal && y < span && x % ROOM_PITCH == 2 {
                        BuildingType::Door
                    } else if (vertical && y % ROOM_PITCH == 1)
                        || (horizontal && y > 0 && y < span && x % ROOM_PITCH == 1)
                    {
                        BuildingType::ScreamingVent
                    } else if vertical || horizontal {
                        BuildingType::Wall
                    } else {
                        BuildingType::Floor
                    };
                    let grid = (origin + bx * pitch + x, origin + by * pitch + y);
                    let pos = WorldMap::grid_to_world(grid.0, grid.1);
                    let e = commands
                        .spawn((
                            Building {
                                kind,
                                is_provisional: false,
                            },
                            Transform::from_translation(pos.extend(0.0)),
                        ))
                        .id();
                    map.register_completed_building_footprint(kind, e, [grid]);
                    if mixed && grid == (origin, origin + 1) {
                        pinned_vent = Some(e);
                        commands.insert_resource(hw_core::selection::SelectedEntity(Some(e)));
                        commands.insert_resource(hw_ui::panels::info_panel::InfoPanelPinState {
                            entity: Some(e),
                        });
                    }
                    if kind == BuildingType::Door {
                        commands.entity(e).insert(Door {
                            state: DoorState::Closed,
                        });
                        doors.push((e, grid));
                    }
                    crate::systems::jobs::attach_building_shell(
                        commands, e, kind, false, pos, assets, handles,
                    );
                }
            }
        }
    }
    // Identical exterior-terminal count on both layouts. Place equipment over
    // existing room floors as in the ordinary construction completion path.
    let generator = (mixed || schedule == "1").then(|| {
        let low = WorldMap::grid_to_world(origin - 2, origin - 2);
        let high = WorldMap::grid_to_world(
            origin + (blocks - 1) * pitch + span + 2,
            origin + (blocks - 1) * pitch + span + 2,
        );
        let yard = commands
            .spawn(hw_world::Yard {
                min: low.min(high),
                max: low.max(high),
            })
            .id();
        let site = commands
            .spawn((
                hw_world::Site {
                    min: low.min(high),
                    max: low.max(high),
                },
                hw_world::PairedYard(yard),
            ))
            .id();
        commands.entity(yard).insert(hw_world::PairedSite(site));
        for row in 0..ROOMS_PER_SIDE {
            let y = origin + (row / side) * pitch + (row % side) * ROOM_PITCH + 1;
            for x in [origin + 1, origin + (blocks - 1) * pitch + span - 1] {
                let kind = BuildingType::Equipment(if x == origin + 1 {
                    hw_jobs::Equipment::SupplyFan
                } else {
                    hw_jobs::Equipment::ExhaustFan
                });
                let pos = WorldMap::grid_to_world(x, y);
                let fan = commands
                    .spawn((
                        Building {
                            kind,
                            is_provisional: false,
                        },
                        Transform::from_translation(pos.extend(0.)),
                        hw_energy::PowerConsumer { demand: 0.5 },
                    ))
                    .id();
                if mixed {
                    commands
                        .entity(fan)
                        .insert(crate::systems::ventilation::climate::ClimateSetting(1));
                    adjustable_fan.get_or_insert(fan);
                }
                map.register_completed_building_footprint(kind, fan, [(x, y)]);
                crate::systems::jobs::attach_building_shell(
                    commands, fan, kind, false, pos, assets, handles,
                );
            }
        }
        commands
            .spawn((
                hw_energy::PowerGenerator {
                    current_output: 80.,
                    ..default()
                },
                Transform::from_translation(low.extend(0.)),
            ))
            .id()
    });
    for mut camera in cameras.iter_mut() {
        let center = WorldMap::grid_to_world(50, 50);
        camera.translation.x = center.x;
        camera.translation.y = center.y;
    }
    commands.insert_resource(Fixture {
        doors,
        side,
        generator,
        adjustable_fan,
        last_climate_tick: 0,
        transitions: 0,
        samples: Vec::new(),
        measure_builds: None,
    });
    commands.insert_resource(AirPerf {
        mode: crate::systems::ventilation::perf::Mode::requested(),
        pinned_vent,
        full_publication: match std::env::var("HW_AIR_PUBLICATION").as_deref() {
            Ok("full") => true,
            Ok("local") | Err(std::env::VarError::NotPresent) => false,
            _ => panic!("unknown HVAC publication mode"),
        },
        use_tape: std::env::var("HW_AIR_COMPARE").as_deref() == Ok("1"),
        dimensions_requested: config.workload() == super::PerfWorkload::VentilationDimensions,
        ..default()
    });
}

pub(crate) fn operate_doors(
    config: Res<PerfScenarioConfig>,
    capture: Res<PerfCapture>,
    time: Res<Time<Virtual>>,
    mut fixture: ResMut<Fixture>,
    mut doors: Query<&mut Door>,
    mut map: WorldMapWrite,
    (mut generators, mut settings): (
        Query<&mut hw_energy::PowerGenerator>,
        Query<&mut crate::systems::ventilation::climate::ClimateSetting>,
    ),
) {
    let measuring = matches!(capture.phase, PerfCapturePhase::Measure);
    if let Some(entity) = fixture.generator
        && (measuring || fixture.adjustable_fan.is_some())
    {
        let second = capture.measure_virtual_secs as u64;
        let powered = if let Some(fan) = fixture.adjustable_fan {
            let phase = fixture.last_climate_tick % 60;
            let requested = if (7..17).contains(&phase) { 2 } else { 1 };
            let mut setting = settings.get_mut(fan).expect("fixture fan setting");
            if setting.0 != requested {
                setting.0 = requested;
            }
            !(41..51).contains(&phase)
        } else {
            (second / 20).is_multiple_of(2)
        };
        let output = if powered { 80. } else { 0. };
        let mut generator = generators.get_mut(entity).expect("benchmark generator");
        if generator.current_output != output {
            generator.current_output = output;
        }
    }
    if !measuring {
        return;
    }
    let target = (f64::from(config.measure_secs()) * 4.0 / SECONDS_PER_TRANSITION).round() as usize;
    let due = ((capture.measure_virtual_secs + time.delta_secs_f64()) / SECONDS_PER_TRANSITION)
        .floor() as usize;
    while fixture.transitions < due.min(target) {
        let slot = fixture.transitions;
        let (entity, grid) = fixture.doors[(slot / 2) % fixture.doors.len()];
        let next = if slot.is_multiple_of(2) {
            DoorState::Open
        } else {
            DoorState::Closed
        };
        let mut door = doors.get_mut(entity).expect("fixture door is present");
        assert_ne!(door.state, next, "another driver changed a benchmark door");
        hw_world::apply_door_state(&mut door, &mut map, grid, next);
        assert_eq!(map.door_state(grid.0, grid.1), Some(next));
        fixture.transitions += 1;
    }
}

impl Fixture {
    pub(super) fn observe(
        &mut self,
        capture: &PerfCapture,
        probe: &mut AirPerf,
        config: &PerfScenarioConfig,
        time: &Time<Virtual>,
        real: &Time<Real>,
    ) {
        self.last_climate_tick = probe.climate_tick;
        let phase = match capture.phase {
            PerfCapturePhase::Warmup
                if capture.warmup_real_secs
                    >= (f64::from(config.warmup_secs()) - 10.0).max(0.0) =>
            {
                "idle"
            }
            PerfCapturePhase::Measure => "doors",
            _ => return,
        };
        assert!(!time.is_paused() && time.relative_speed_f64() == 4.0);
        if phase == "doors" {
            let expected = self.measure_builds.get_or_insert(probe.rebuilds);
            assert_eq!(*expected, probe.rebuilds, "door churn rebuilt air geometry");
        }
        let process_cpu_ns = crate::systems::ventilation::perf::cpu_time_ns(true);
        let transport = if phase == "doors" && probe.dimensions_requested {
            let elapsed = capture.measure_virtual_secs + time.delta_secs_f64();
            let limit = (4.0 * f64::from(config.measure_secs())).round() as usize;
            Some(
                probe
                    .transport
                    .as_mut()
                    .expect("missing resolved transport geometry")
                    .observe(elapsed, limit),
            )
        } else {
            None
        };
        self.samples.push(Sample {
            transport,
            phase,
            real_delta_ms: real.delta_secs_f64() * 1000.0,
            cpu_ns: probe.latest_cpu_ns,
            climate_cpu_ns: probe.climate_cpu_ns,
            climate_tick: probe.climate_tick,
            climate_zones: probe.climate_zones,
            process_cpu_ns,
            prepare_ns: probe.climate_prepare_ns,
            step_ns: probe.climate_step_ns,
            publish_ns: probe.climate_publish_ns,
            powered_fans: probe.synced_fans,
            high_fans: probe.high_fans,
            wind: probe.wind,
            inspected_net: probe.inspected_net,
            input_ns: probe.input_ns,
            rooms_written: probe.rooms_written,
            vents_written: probe.vents_written,
            groups_changed: probe.groups_changed,
            publish_allocations: probe.publish_allocations,
            publish_bytes: probe.publish_bytes,
            inspection_ns: probe.inspection_ns,
            inspections: probe.inspections,
            exterior_net_abs: probe.exterior_net_abs,
            transitions: self.transitions,
            speed: time.relative_speed_f64(),
            calls: probe.calls,
            rebuilds: probe.rebuilds,
        });
    }

    pub(super) fn write(
        &self,
        config: &PerfScenarioConfig,
        probe: &AirPerf,
    ) -> std::io::Result<()> {
        let room_count = ROOMS_PER_SIDE.pow(2) as usize;
        let group_rooms = self.side.pow(2) as usize;
        let groups = room_count / group_rooms;
        let expected =
            (f64::from(config.measure_secs()) * 4.0 / SECONDS_PER_TRANSITION).round() as usize;
        if probe.rooms != room_count
            || self.doors.len() != room_count
            || probe.counts.room_groups != groups
            || probe.counts.largest_room_group != group_rooms
            || probe.counts.matrix_solves != 2 * groups
            || self.transitions != expected
            || self.measure_builds != Some(probe.rebuilds)
        {
            return Err(std::io::Error::other(format!(
                "ventilation fixture mismatch: rooms={}, doors={}, groups={:?}, transitions={}/{expected}",
                probe.rooms,
                self.doors.len(),
                probe.counts,
                self.transitions
            )));
        }
        let mut report = serde_json::json!({"mode":probe.actual_mode,"full_publication":probe.full_publication,"mixed":self.adjustable_fan.is_some(),"checked_steps":probe.checked_steps,"climate_build_ns":probe.climate_build_ns,"rooms":probe.rooms,"doors":self.doors.len(),"openings":probe.grates,"groups":groups,
            "rooms_per_group":group_rooms,"matrix_solves_at_build":probe.counts.matrix_solves,"build_cpu_ns":probe.build_cpu_ns,
            "transitions":self.transitions,"samples":self.samples});
        if self.adjustable_fan.is_some() {
            std::fs::write(
                super::output::perf_output_directory(config).join("publication-trace.bin"),
                &probe.publication_trace,
            )?;
        }
        // Linux process memory after capture, outside the measurement window.
        let status = std::fs::read_to_string("/proc/self/status")?;
        for (field, label) in [("rss_kib", "VmRSS:"), ("peak_rss_kib", "VmHWM:")] {
            let value = status
                .lines()
                .find(|line| line.starts_with(label))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or_else(|| std::io::Error::other("missing process RSS"))?;
            report[field] = value.into();
        }
        if let Some(transport) = &probe.transport {
            report["transport"] = transport.report();
        }
        std::fs::write(
            super::output::perf_output_directory(config).join("ventilation.json"),
            serde_json::to_vec(&report)?,
        )
    }
}
