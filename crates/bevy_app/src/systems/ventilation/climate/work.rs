//! Per-step working storage, device operations and room observations.
#[cfg(feature = "profiling")]
use super::StageTimes;
use super::{AirInputs, AirLedger, DeviceReading, RoomClimate, above, below, chimney_drive};
use crate::systems::ventilation::services::{Grid, Services, neighbors};
use crate::systems::ventilation::solver::Solver;
use bevy::prelude::*;
use hw_infra::air_transport::{Fields, saturation};
use hw_infra::ventilation::{AirBoundary, OpeningFlow, Ventilation, VentilationState};
use hw_jobs::Equipment;
use std::collections::{HashMap, HashSet};

#[derive(Default, Clone)]
struct Operation {
    rejected_heat: f64,
    heat: f64,
    humidify: f64,
    dry: f64,
    filters: f64,
    recovery: f64,
    basin: bool,
    water: bool,
}

#[derive(Default)]
pub(super) struct AirWork {
    values: Vec<Fields>,
    sources: Vec<Fields>,
    loads: Vec<(usize, Grid, Fields)>,
    drives: Vec<f64>,
    removal: Vec<f64>,
    ops: Vec<Operation>,
    intake: Vec<f64>,
    exhaust: Vec<f64>,
    previous: Vec<f64>,
    supplied: HashSet<usize>,
    capture: HashMap<(usize, Grid), f64>,
}

impl AirWork {
    pub(super) fn collect_inputs(
        &mut self,
        inputs: &AirInputs,
        svc: &Services,
        ledger: &AirLedger,
        #[cfg(feature = "profiling")] fixed_occupants: bool,
    ) {
        let n = svc.layout.floors.len();
        let Self {
            values,
            supplied,
            sources,
            loads,
            ..
        } = self;
        values.clear();
        values.extend(ledger.zones.iter().map(|z| z.air));
        supplied.clear();
        supplied.extend(
            svc.devices
                .iter()
                .filter(|d| d.kind == Equipment::Intake && inputs.powered(d.entity))
                .flat_map(|d| d.pipes.iter().copied()),
        );
        sources.clear();
        sources.resize(n, [0.; 3]);
        loads.clear();
        for pose in &inputs.souls {
            let grid = hw_world::WorldMap::world_to_grid(pose.translation.xy());
            if let Some(&i) = svc.zones.get(&grid) {
                loads.push((i, grid, [0.24, 0.04, 0.045]));
            }
        }
        #[cfg(feature = "profiling")]
        if fixed_occupants && svc.layout.rooms.len() == 256 {
            super::measurement::set_occupancy(
                loads,
                ledger.tick,
                &svc.room_tiles[..svc.layout.rooms.len()],
            );
        }
        for &(i, p) in &svc.emitters {
            loads.push((i, p, [0.4, 0.08, 0.09]));
        }
    }

    pub(super) fn operate_devices(
        &mut self,
        commands: &mut Commands,
        inputs: &AirInputs,
        svc: &Services,
        ledger: &AirLedger,
        outside: Fields,
        #[cfg(feature = "profiling")] mut timing: Option<&mut StageTimes>,
    ) {
        let n = svc.layout.floors.len();
        let Self {
            ops,
            drives,
            supplied,
            ..
        } = self;
        let openings = &svc.vents.openings;
        drives.clear();
        drives.resize(openings.len(), 0.);
        ops.clear();
        ops.resize(n, Operation::default());
        for d in &svc.devices {
            #[cfg(feature = "profiling")]
            if matches!(d.kind, Equipment::SupplyFan | Equipment::ExhaustFan)
                && d.terminal.is_some()
                && inputs.powered(d.entity)
                && let Some(t) = &mut timing
            {
                t.supplied += 1;
                t.high_fans += usize::from(inputs.settings.get(d.entity).is_ok_and(|s| s.0 == 2));
            }
            let water = d.pipes.iter().any(|id| supplied.contains(id));
            let on = d.kind.demand() == 0.0 || inputs.powered(d.entity);
            let mut state = if on {
                "稼働可能"
            } else {
                "給電待ち（工区の対応Yardと配電を確認）"
            };
            if let Some(i) = d.room {
                let z = &ledger.zones[i];
                let demand = match d.kind {
                    Equipment::SupplyFan | Equipment::ExhaustFan | Equipment::Hood
                        if d.terminal.is_some() =>
                    {
                        let command = inputs.settings.get(d.entity).copied().unwrap_or_default();
                        d.kind.demand() * (command.drive(z.boost) as f32 / 0.85)
                    }
                    Equipment::HeatPump if z.thermal == 0 || !d.condenser => 0.10,
                    _ => d.kind.demand(),
                };
                if inputs
                    .consumers
                    .get(d.entity)
                    .is_ok_and(|c| (c.demand - demand).abs() > 1e-4_f32)
                {
                    let target = d.entity;
                    commands.queue(move |world: &mut World| {
                        if let Some(mut c) = world.get_mut::<hw_energy::PowerConsumer>(target) {
                            c.demand = demand;
                        }
                    });
                }
                let op = &mut ops[i];
                match d.kind {
                    Equipment::SupplyFan
                    | Equipment::ExhaustFan
                    | Equipment::Hood
                    | Equipment::Chimney => {
                        if let Some((j, length)) = d.terminal {
                            if on {
                                let setting =
                                    inputs.settings.get(d.entity).copied().unwrap_or_default();
                                let drive = match d.kind {
                                    Equipment::Chimney => chimney_drive(z.air[0] - outside[0]),
                                    Equipment::SupplyFan => setting.drive(z.boost),
                                    _ => -setting.drive(z.boost),
                                };
                                if matches!(openings[j].b, AirBoundary::Outdoor { .. }) {
                                    drives[j] += drive / (1.0 + 0.20 * length as f64);
                                }
                                state = "外壁の風穴に接続";
                            }
                        } else {
                            state = "屋外端末なし（風穴まで風道を接続）";
                        }
                    }
                    Equipment::Basin => {
                        op.basin = true;
                        op.water |= water;
                        state = if water {
                            "給水・排泥中"
                        } else {
                            "断水・停止"
                        };
                    }
                    Equipment::Filter if on => {
                        op.filters += 0.65;
                        state = "循環ろ過中";
                    }
                    Equipment::HeatPump => {
                        if !d.condenser {
                            state = "直近の屋外に室外機が必要";
                        } else if on {
                            op.heat += f64::from(z.thermal) * 3.0;
                            if z.thermal != 0 {
                                op.rejected_heat += 1.0 - f64::from(z.thermal) * 3.0;
                            }
                            state = match z.thermal {
                                -1 => "冷房中（除熱と消費電力を屋外へ放熱）",
                                1 => "暖房中",
                                _ => "設定温度内・待機",
                            };
                        }
                    }
                    Equipment::Humidifier | Equipment::Dehumidifier => {
                        if !water {
                            state = "給排水接続待ち";
                        } else if on {
                            let rh = z.air[1] / saturation(z.air[0]);
                            if d.kind == Equipment::Humidifier && below(rh, 0.40) {
                                op.humidify += 0.25;
                                state = "加湿中";
                            } else if d.kind == Equipment::Dehumidifier && above(rh, 0.60) {
                                op.dry += 0.25;
                                state = "除湿・排水中";
                            } else {
                                state = "設定湿度内・待機";
                            }
                        }
                    }
                    Equipment::Recuperator => {
                        op.recovery = 0.6;
                        state = "給排気の顕熱を回収";
                    }
                    _ => {}
                }
            } else if d.kind.indoor() {
                state = "成立した部屋の床が必要";
            } else if d.kind == Equipment::OutdoorUnit && !svc.layout.is_outside(d.grid) {
                state = "屋外への排熱経路なし";
            }
            if !inputs.current.get(d.entity).is_ok_and(|r| r.0 == state) {
                commands.entity(d.entity).try_insert(DeviceReading(state));
            }
        }
    }

    pub(super) fn collect_sources(
        &mut self,
        inputs: &AirInputs,
        svc: &Services,
        flows: &[OpeningFlow],
        #[cfg(feature = "profiling")] mut timing: Option<&mut StageTimes>,
    ) {
        let openings = &svc.vents.openings;
        let n = svc.layout.floors.len();
        let Self {
            intake,
            exhaust,
            capture,
            loads,
            sources,
            previous,
            values,
            ..
        } = self;
        intake.clear();
        exhaust.clear();
        intake.resize(n, 0.);
        exhaust.resize(n, 0.);
        for (p, flow) in openings.iter().zip(flows) {
            if matches!(p.b, AirBoundary::Outdoor { .. }) {
                intake[p.a] += (-flow.net).max(0.0);
                exhaust[p.a] += flow.net.max(0.0);
                #[cfg(feature = "profiling")]
                if let Some(stages) = &mut timing {
                    stages.exterior_net_abs += flow.net.abs();
                }
            }
        }
        capture.clear();
        for d in &svc.devices {
            if d.kind == Equipment::Hood
                && inputs.powered(d.entity)
                && let (Some(i), Some((j, _))) = (d.room, d.terminal)
            {
                let fraction = flows[j].net.clamp(0.0, 1.0) * 0.8;
                for tile in std::iter::once(d.grid).chain(neighbors(d.grid)) {
                    let entry = capture.entry((i, tile)).or_default();
                    *entry = entry.max(fraction);
                }
            }
        }
        for &(i, p, load) in loads.iter() {
            sources[i][0] += load[0];
            sources[i][1] += load[1];
            sources[i][2] += load[2] * (1.0 - capture.get(&(i, p)).copied().unwrap_or(0.0));
        }
        previous.clear();
        previous.extend(values.iter().map(|v| v[2]));
    }

    pub(super) fn integrate(
        &mut self,
        svc: &Services,
        ledger: &mut AirLedger,
        solver: &mut Solver,
        outside: Fields,
        #[cfg(feature = "profiling")] timing: Option<&mut StageTimes>,
        #[cfg(feature = "profiling")] tape: Option<
            &crate::systems::ventilation::replay::Trajectory,
        >,
    ) {
        let Self {
            ops,
            values,
            intake,
            exhaust,
            sources,
            ..
        } = self;
        for (i, op) in ops.iter().enumerate() {
            let volume = svc.volumes[i].max(1) as f64;
            let v = values[i];
            // Limit each local sink to stored mass; recovery transfers sensible heat only.
            let dry = op.dry.min(v[1] * volume);
            sources[i][1] += op.humidify - dry;
            sources[i][0] += op.heat
                + 2.45 * (dry - op.humidify)
                + op.recovery * intake[i].min(exhaust[i]) * (v[0] - outside[0]);
        }
        #[cfg(feature = "profiling")]
        super::measurement::measured_transport(
            solver,
            ledger.tick,
            outside,
            values,
            sources,
            timing,
            tape,
        );
        #[cfg(not(feature = "profiling"))]
        solver.step(ledger.tick, outside, values, sources);
        for (z, air) in ledger.zones.iter_mut().zip(values.iter_mut()) {
            let condensate = (air[1] - saturation(air[0])).max(0.0);
            air[1] -= condensate;
            air[0] += condensate * 2.45 / 1.5;
            z.air = *air;
        }
    }

    pub(super) fn publish_rooms<'a>(
        &self,
        commands: &mut Commands,
        inputs: &AirInputs,
        ledger: &mut AirLedger,
        rooms: impl Iterator<Item = (Entity, &'a Ventilation)>,
        outside: Fields,
        live: bool,
    ) {
        let Self {
            ops,
            values,
            previous,
            ..
        } = self;

        for (i, (entity, rate)) in rooms.enumerate() {
            let op = &ops[i];
            let sanction = op.water && rate.state() == VentilationState::Ventilated;
            let zone = &mut ledger.zones[i];
            if live && sanction && zone.sanction == Some(false) {
                if let Some(assets) = inputs.assets.as_ref() {
                    let (x, y) = zone.tiles[zone.tiles.len() / 2];
                    hw_visual::floating_text::spawn_floating_text(
                        commands,
                        "認可",
                        hw_world::WorldMap::grid_to_world(x, y)
                            .extend(hw_core::constants::Z_FLOATING_TEXT),
                        hw_visual::floating_text::FloatingTextConfig {
                            initial_color: Color::srgb(0.5, 1.0, 0.72),
                            ..default()
                        },
                        None,
                        assets.font_ui.clone(),
                    );
                }
                zone.stamp_until = ledger.tick.saturating_add(5);
            }
            zone.sanction = Some(sanction);
            commands.entity(entity).try_insert(RoomClimate {
                granted: sanction && ledger.tick < zone.stamp_until,
                rejected_heat: op.rejected_heat,
                air: values[i],
                outdoor: outside,
                trend: values[i][2] - previous[i],
                basin: op.basin,
                water: op.water,
                sanctioned: sanction,
                boost: zone.boost,
            });
        }
    }

    pub(super) fn flow_inputs(&mut self) -> (&[f64], &[f64]) {
        self.removal.clear();
        self.removal.extend(self.ops.iter().map(|op| op.filters));
        (&self.drives, &self.removal)
    }
}
