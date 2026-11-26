//! Durable room air and fixed-step HVAC operation. Layout and numerical factors are derived.
use super::services::{Grid, Services};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_infra::air_transport::{Fields, saturation, weather};
use hw_infra::ventilation::{AirBoundary, VentilationState};
use hw_jobs::Equipment;
use std::collections::{HashMap, HashSet};

#[derive(Reflect, Clone, Default)]
pub(crate) struct AirZone {
    tiles: Vec<Grid>,
    junction: bool,
    air: Fields,
    boost: bool,
    thermal: i8,
    sanction: Option<bool>,
    stamp_until: u64,
}

// Treat round-off at controller thresholds as equality. The transported state
// retains full precision; the tolerance applies only to discrete decisions.
fn above(value: f64, limit: f64) -> bool {
    value - limit > 1e-9
}
fn below(value: f64, limit: f64) -> bool {
    above(limit, value)
}

fn chimney_drive(temperature_difference: f64) -> f64 {
    let levels = temperature_difference * 0.30;
    let magnitude = (levels.abs() + 0.5 + 1e-9).floor();
    -levels.signum() * magnitude * 0.25
}

impl AirZone {
    fn update_controls(&mut self, outside: Fields) {
        let t = self.air[0];
        if above(t, 25.) {
            self.thermal = -1;
        } else if below(t, 19.) {
            self.thermal = 1;
        } else if (self.thermal < 0 && below(t, 23.)) || (self.thermal > 0 && above(t, 21.)) {
            self.thermal = 0;
        }
        if above(self.air[2], 0.15)
            || (above(t, 24.) && above(t, outside[0] + 2.) && below(outside[1], self.air[1] + 1.))
        {
            self.boost = true;
        } else if below(self.air[2], 0.09)
            && (below(t, 23.) || !above(t, outside[0] + 1.) || above(outside[1], self.air[1] + 2.))
        {
            self.boost = false;
        }
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub(crate) struct AirLedger {
    zones: Vec<AirZone>,
    pub(crate) tick: u64,
    remainder: f64,
}

impl AirLedger {
    pub(crate) fn valid(&self) -> bool {
        let mut tiles = HashSet::new();
        self.remainder.is_finite()
            && (0.0..1.0).contains(&self.remainder)
            && self.zones.iter().all(|z| {
                !z.tiles.is_empty()
                    && z.tiles.iter().all(|&p| {
                        (0..hw_core::constants::MAP_WIDTH).contains(&p.0)
                            && (0..hw_core::constants::MAP_HEIGHT).contains(&p.1)
                            && tiles.insert(p)
                    })
                    && z.air.iter().all(|x| x.is_finite())
                    && z.air[1] >= 0.0
                    && z.air[2] >= 0.0
                    && (-1..=1).contains(&z.thermal)
            })
    }

    fn remap(&mut self, tiles: &[Vec<Grid>], volumes: &[usize], ambient: Fields) {
        if tiles.len() == self.zones.len()
            && self
                .zones
                .iter()
                .zip(tiles)
                .zip(volumes)
                .all(|((z, t), &v)| z.tiles == *t && z.junction == (v == 0))
        {
            return;
        }
        let mut old = HashMap::new();
        for zone in self.zones.iter().filter(|z| !z.junction) {
            for &tile in &zone.tiles {
                old.insert(tile, zone);
            }
        }
        self.zones = tiles
            .iter()
            .zip(volumes)
            .map(|(t, &volume)| {
                let mut sum = [0.0; 3];
                let same = old.get(&t[0]).filter(|z| z.tiles == *t);
                if let Some(z) = same
                    && z.junction == (volume == 0)
                {
                    return (*z).clone();
                }
                for p in t {
                    let values = old.get(p).map_or(ambient, |z| z.air);
                    for (s, v) in sum.iter_mut().zip(values) {
                        *s += v;
                    }
                }
                AirZone {
                    junction: volume == 0,
                    tiles: t.clone(),
                    air: sum.map(|s| s / t.len() as f64),
                    boost: same.is_some_and(|z| z.boost),
                    sanction: same.and_then(|z| z.sanction),
                    stamp_until: same.map_or(0, |z| z.stamp_until),
                    thermal: same.map_or(0, |z| z.thermal),
                }
            })
            .collect();
    }
}

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) struct RoomClimate {
    air: Fields,
    outdoor: Fields,
    trend: f64,
    basin: bool,
    water: bool,
    sanctioned: bool,
    rejected_heat: f64,
    granted: bool,
    boost: bool,
}
impl RoomClimate {
    pub(crate) fn text(self) -> String {
        let drain = match (self.basin, self.water) {
            (false, _) => "浄化盤なし",
            (true, false) => "断水",
            (true, true) => "排泥中",
        };
        format!(
            "室温 {:.1}℃　湿度 {:.0}%（{:.1} g/m³）\n瘴気 {:.3}（{:+.3}/秒）　外気 {:.1}℃\n排泥 {}　認可 {}\n標準運転の増風要求　{}\n室外機の放熱 {:+.1}/秒",
            self.air[0],
            100.0 * self.air[1] / saturation(self.air[0]),
            self.air[1],
            self.air[2],
            self.trend,
            self.outdoor[0],
            drain,
            if self.granted {
                "認可印・新規取得"
            } else if self.sanctioned {
                "取得"
            } else {
                "未達"
            },
            if self.boost { "あり" } else { "なし" },
            self.rejected_heat
        )
    }
}
#[derive(Component, PartialEq)]
pub(crate) struct DeviceReading(pub(crate) &'static str);

#[derive(Default)]
pub(crate) struct AirRuntime {
    factors: Option<super::solver::Solver>,
    work: AirWork,
    #[cfg(feature = "profiling")]
    full_publication: bool,
    #[cfg(feature = "profiling")]
    inspected_port: Option<usize>,
    patterns: hw_infra::ventilation::network::PatternCache,
    #[cfg(feature = "profiling")]
    mode: super::perf::Mode,
    #[cfg(feature = "profiling")]
    timing: Option<StageTimes>,
    #[cfg(feature = "profiling")]
    tape: Option<std::sync::Arc<super::replay::Trajectory>>,
}

#[cfg(feature = "profiling")]
#[derive(Default)]
struct StageTimes {
    prepare: u64,
    step: u64,
    publish: u64,
    input: u64,
    rooms_written: usize,
    vents_written: usize,
    groups_changed: usize,
    publish_allocations: u64,
    publish_bytes: u64,
    wind: f64,
    inspected_net: f64,
    supplied: usize,
    high_fans: usize,
    exterior_net_abs: f64,
    checked: u64,
}
type DeviceInputsChanged = Or<(
    Changed<hw_energy::PowerSupplyState>,
    Changed<ClimateSetting>,
)>;

#[derive(SystemParam)]
pub(crate) struct AirInputs<'w, 's> {
    assets: Option<Res<'w, crate::assets::GameAssets>>,
    services: Option<Res<'w, Services>>,
    power: Query<'w, 's, &'static hw_energy::PowerSupplyState>,
    souls: Query<'w, 's, &'static Transform, With<hw_core::soul::DamnedSoul>>,
    current: Query<'w, 's, &'static DeviceReading>,
    updates: Query<'w, 's, (), DeviceInputsChanged>,
    consumers: Query<'w, 's, &'static hw_energy::PowerConsumer>,
    settings: Query<'w, 's, &'static ClimateSetting>,
}

#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component, Default)]
pub(crate) struct ClimateSetting(pub(crate) u8);

impl ClimateSetting {
    pub(crate) fn label(self) -> &'static str {
        ["標準", "省エネ", "強運転"][self.0 as usize % 3]
    }
    fn drive(self, boost: bool) -> f64 {
        match self.0 {
            1 => 0.5,
            2 => 2.5,
            _ if boost => 2.0,
            _ => 0.85,
        }
    }
}

pub(crate) fn elapsed(time: Res<Time<Virtual>>) -> f64 {
    if time.is_paused() {
        0.0
    } else {
        time.delta_secs_f64()
    }
}

pub(crate) fn advance(
    In(dt): In<f64>,
    mut commands: Commands,
    inputs: AirInputs,
    mut ledgers: Query<&mut AirLedger>,
    mut cache: Local<AirRuntime>,
    #[cfg(feature = "profiling")] probe: Option<ResMut<super::perf::AirPerf>>,
) {
    let Some(services) = inputs.services.as_ref() else {
        return;
    };
    let fresh = services.is_changed() || cache.factors.is_none();
    if fresh {
        #[cfg(feature = "profiling")]
        {
            cache.inspected_port = probe.as_ref().and_then(|p| p.pinned_vent).and_then(|e| {
                services
                    .vent_entities
                    .iter()
                    .position(|(entity, _)| *entity == e)
            });
            cache.full_publication = probe.as_ref().is_some_and(|p| p.full_publication);
            cache.mode = probe.as_ref().map(|p| p.mode).unwrap_or_default();
            cache.tape = probe
                .as_ref()
                .filter(|p| {
                    p.use_tape && services.volumes.len() == 256 && services.ports.len() == 512
                })
                .map(|_| std::sync::Arc::new(super::replay::Trajectory::load(services)));
        }
        cache.factors = None;
    }
    let mut initial = AirLedger::default();
    let mut stored = ledgers.single_mut().ok();
    if stored.is_none() && services.room_tiles.is_empty() {
        return;
    }
    let ledger = stored.as_deref_mut().unwrap_or(&mut initial);
    #[cfg(feature = "profiling")]
    let timing = probe.map(|probe| (probe, super::perf::cpu_time_ns(false)));
    if fresh {
        ledger.remap(
            &services.room_tiles,
            &services.volumes,
            weather(ledger.tick).0,
        );
    }
    ledger.remainder += dt;
    if ledger.remainder < 1.0 && !fresh && inputs.updates.is_empty() {
        return;
    }
    #[cfg(feature = "profiling")]
    {
        cache.timing = timing.as_ref().map(|_| StageTimes::default());
    }
    let steps = ledger.remainder.floor() as usize;
    ledger.remainder -= steps as f64;
    for step in 0..steps.max(1) {
        let integrate = step < steps;
        let outside = weather(ledger.tick);
        evaluate(
            &mut commands,
            &inputs,
            services,
            ledger,
            &mut cache,
            outside,
            (integrate, dt > 0.0),
        );
        if integrate {
            ledger.tick += 1;
        }
    }
    #[cfg(feature = "profiling")]
    if let Some((mut probe, start)) = timing {
        let cost = super::perf::cpu_time_ns(false) - start;
        probe.climate_cpu_ns += cost;
        if fresh && services.volumes.len() == 256 {
            probe.climate_build_ns = cost;
        }
        let stages = cache.timing.as_ref().unwrap();
        probe.climate_prepare_ns += stages.prepare;
        probe.climate_step_ns += stages.step;
        probe.climate_publish_ns += stages.publish;
        probe.input_ns += stages.input;
        probe.rooms_written += stages.rooms_written;
        probe.vents_written += stages.vents_written;
        probe.groups_changed += stages.groups_changed;
        probe.publish_allocations += stages.publish_allocations;
        probe.publish_bytes += stages.publish_bytes;
        probe.wind = stages.wind;
        probe.inspected_net = stages.inspected_net;
        probe.synced_fans = stages.supplied;
        probe.high_fans = stages.high_fans;
        probe.exterior_net_abs = stages.exterior_net_abs;
        probe.checked_steps += stages.checked;
        probe.actual_mode = cache.factors.as_ref().unwrap().mode();
        probe.climate_tick = ledger.tick;
        if probe.pinned_vent.is_some() && steps > 0 && (130..330).contains(&ledger.tick) {
            let (rates, flows) = cache.factors.as_ref().unwrap().readings();
            let trace = &mut probe.publication_trace;
            trace.extend_from_slice(&ledger.tick.to_le_bytes());
            for (z, rate) in ledger.zones.iter().zip(rates) {
                for value in
                    z.air
                        .into_iter()
                        .chain([rate.supplied(), rate.outdoor_in, rate.transfer_in])
                {
                    trace.extend_from_slice(&value.to_le_bytes());
                }
                trace.extend([
                    z.boost as u8,
                    z.thermal as u8,
                    u8::from(z.sanction == Some(true)),
                    rate.state() as u8,
                ]);
            }
            for f in flows {
                trace.extend_from_slice(&f.net.to_le_bytes());
            }
        }
        probe.climate_zones = ledger.zones.len();
    }
    if stored.is_none() {
        commands.spawn(initial);
    }
}

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
struct AirWork {
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

fn evaluate(
    commands: &mut Commands,
    inputs: &AirInputs,
    svc: &Services,
    ledger: &mut AirLedger,
    cache: &mut AirRuntime,
    (outside, wind): (Fields, f64),
    (integrate, live): (bool, bool),
) {
    let layout = &svc.layout;
    let n = layout.floors.len();
    #[cfg(feature = "profiling")]
    let input_start = cache
        .timing
        .as_ref()
        .map(|_| super::perf::cpu_time_ns(false));
    let AirWork {
        values,
        sources,
        loads,
        drives,
        removal,
        ops,
        intake,
        exhaust,
        previous,
        supplied,
        capture,
    } = &mut cache.work;
    values.clear();
    values.extend(ledger.zones.iter().map(|z| z.air));
    let powered = |e| {
        inputs
            .power
            .get(e)
            .is_ok_and(|p| *p == hw_energy::PowerSupplyState::Supplied)
    };
    supplied.clear();
    supplied.extend(
        svc.devices
            .iter()
            .filter(|d| d.kind == Equipment::Intake && powered(d.entity))
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
    if cache.inspected_port.is_some() && layout.rooms.len() == 256 {
        // Give both timed arms the same eight occupants at each HVAC tick.
        // Actor AI, movement and rendering still run in the actual world.
        loads.clear();
        for actor in 0..8 {
            let i = (actor * 31 + (ledger.tick / 12) as usize) % layout.rooms.len();
            loads.push((i, svc.room_tiles[i][0], [0.24, 0.04, 0.045]));
        }
    }
    for &(i, p) in &svc.emitters {
        loads.push((i, p, [0.4, 0.08, 0.09]));
    }
    if integrate {
        for zone in &mut ledger.zones {
            zone.update_controls(outside);
        }
    }
    let openings = &svc.ports;
    drives.clear();
    drives.resize(openings.len(), 0.);
    ops.clear();
    ops.resize(n, Operation::default());
    for d in &svc.devices {
        #[cfg(feature = "profiling")]
        if matches!(d.kind, Equipment::SupplyFan | Equipment::ExhaustFan)
            && d.terminal.is_some()
            && powered(d.entity)
            && let Some(t) = &mut cache.timing
        {
            t.supplied += 1;
            t.high_fans += usize::from(inputs.settings.get(d.entity).is_ok_and(|s| s.0 == 2));
        }
        let water = d.pipes.iter().any(|id| supplied.contains(id));
        let on = d.kind.demand() == 0.0 || powered(d.entity);
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
        } else if d.kind == Equipment::OutdoorUnit && !layout.is_outside(d.grid) {
            state = "屋外への排熱経路なし";
        }
        if !inputs.current.get(d.entity).is_ok_and(|r| r.0 == state) {
            commands.entity(d.entity).try_insert(DeviceReading(state));
        }
    }
    #[cfg(feature = "profiling")]
    let prepare_start = cache
        .timing
        .as_ref()
        .map(|_| super::perf::cpu_time_ns(false));
    let solver = cache.factors.get_or_insert_with(|| {
        super::solver::Solver::new(
            svc,
            openings,
            &mut cache.patterns,
            #[cfg(feature = "profiling")]
            cache.mode,
            #[cfg(feature = "profiling")]
            cache.tape.as_ref(),
        )
    });
    removal.clear();
    removal.extend(ops.iter().map(|op| op.filters));
    let changed = solver.update(ledger.tick, wind, drives, removal, svc, openings);
    #[cfg(feature = "profiling")]
    let publish_start = prepare_start.map(|t| {
        let now = super::perf::cpu_time_ns(false);
        let stages = cache.timing.as_mut().unwrap();
        stages.input += t - input_start.unwrap();
        stages.wind = wind;
        stages.prepare += now - t;
        now
    });
    let (_, flows) = solver.readings();
    #[cfg(feature = "profiling")]
    if let Some((timing, i)) = cache.timing.as_mut().zip(cache.inspected_port) {
        timing.inspected_net = flows[i].net;
    }
    if changed {
        #[cfg(feature = "profiling-memory")]
        if publish_start.is_some() {
            hw_core::profiling_alloc_scope::begin();
        }
        #[cfg(feature = "profiling")]
        let full = cache.full_publication;
        #[cfg(not(feature = "profiling"))]
        let full = false;
        let counts = super::publication::publish(solver, svc, commands, full);
        #[cfg(not(feature = "profiling"))]
        let _ = counts;
        #[cfg(feature = "profiling")]
        if let Some(s) = &mut cache.timing {
            s.rooms_written += counts.0;
            s.vents_written += counts.1;
            s.groups_changed += counts.2;
            #[cfg(feature = "profiling-memory")]
            {
                let allocation = hw_core::profiling_alloc_scope::end();
                s.publish_allocations += allocation.calls;
                s.publish_bytes += allocation.bytes;
            }
        }
    }
    #[cfg(feature = "profiling")]
    if let Some(t) = publish_start {
        cache.timing.as_mut().unwrap().publish += super::perf::cpu_time_ns(false) - t;
    }
    intake.clear();
    exhaust.clear();
    intake.resize(n, 0.);
    exhaust.resize(n, 0.);
    for (p, flow) in openings.iter().zip(flows) {
        if matches!(p.b, AirBoundary::Outdoor { .. }) {
            intake[p.a] += (-flow.net).max(0.0);
            exhaust[p.a] += flow.net.max(0.0);
            #[cfg(feature = "profiling")]
            if let Some(stages) = &mut cache.timing {
                stages.exterior_net_abs += flow.net.abs();
            }
        }
    }
    capture.clear();
    for d in &svc.devices {
        if d.kind == Equipment::Hood
            && powered(d.entity)
            && let (Some(i), Some((j, _))) = (d.room, d.terminal)
        {
            let fraction = flows[j].net.clamp(0.0, 1.0) * 0.8;
            for tile in std::iter::once(d.grid).chain(super::services::neighbors(d.grid)) {
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
    if integrate {
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
        let start = cache
            .timing
            .as_ref()
            .map(|_| super::perf::cpu_time_ns(false));
        #[cfg(feature = "profiling")]
        let comparison = cache.tape.as_ref().map(|t| t.frame(ledger.tick));
        #[cfg(feature = "profiling")]
        let sources = comparison.map(|(sources, _)| sources).unwrap_or(sources);
        #[cfg(not(feature = "profiling"))]
        let sources = sources.as_slice();
        solver.step(ledger.tick, outside, values, sources);
        #[cfg(feature = "profiling")]
        if let Some((_, expected)) = comparison {
            assert!(
                values
                    .iter()
                    .flatten()
                    .zip(expected.iter().flatten())
                    .all(|(&a, &b)| (a - b).abs() <= 1e-9 * (1. + b.abs())),
                "reference climate diverged at tick {}",
                ledger.tick
            );
            cache.timing.as_mut().unwrap().checked += 1;
        }
        #[cfg(feature = "profiling")]
        if let Some(t) = start {
            cache.timing.as_mut().unwrap().step += super::perf::cpu_time_ns(false) - t;
        }
        for (z, air) in ledger.zones.iter_mut().zip(values.iter_mut()) {
            let condensate = (air[1] - saturation(air[0])).max(0.0);
            air[1] -= condensate;
            air[0] += condensate * 2.45 / 1.5;
            z.air = *air;
        }
    }
    let (rates, _) = solver.readings();
    for (i, &entity) in layout.rooms.iter().enumerate() {
        let op = &ops[i];
        let sanction = op.water && rates[i].state() == VentilationState::Ventilated;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::ventilation::{RoomVentilation, update_room_ventilation};
    use bevy::ecs::system::RunSystemOnce;
    use hw_jobs::{Building, BuildingType};
    use hw_world::room_detection::{RoomBounds, RoomTileSignature};
    use hw_world::{Room, RoomDetectionState, WorldMap};

    #[test]
    fn controller_roundoff_at_thresholds_preserves_history() {
        let outside = [24., 9., 0.];
        for noise in [-4e-14, 0., 4e-14] {
            let mut zone = AirZone {
                air: [25. + noise, 9., 0.15 + noise],
                ..default()
            };
            zone.update_controls(outside);
            assert_eq!((zone.thermal, zone.boost), (0, false));
            zone.air = [25.00001, 9., 0.150001];
            zone.update_controls(outside);
            assert_eq!((zone.thermal, zone.boost), (-1, true));
            zone.air = [23. + noise, 9., 0.09 + noise];
            zone.update_controls(outside);
            assert_eq!((zone.thermal, zone.boost), (-1, true));
            zone.air = [22.99999, 9., 0.089999];
            zone.update_controls(outside);
            assert_eq!((zone.thermal, zone.boost), (0, false));
            assert_eq!(chimney_drive(5. + noise), -0.5);
            assert_eq!(chimney_drive(-5. + noise), 0.5);
        }
    }

    #[test]
    fn remodeling_mixes_the_old_floor_volume_with_outdoor_air() {
        let mut ledger = AirLedger::default();
        ledger.remap(&[vec![(1, 1), (1, 2)], vec![(2, 1)]], &[2, 1], [10.0; 3]);
        ledger.zones[0].air = [24.0, 8.0, 0.4];
        ledger.zones[1].air = [12.0, 4.0, 0.1];
        ledger.remap(
            &[vec![(1, 1), (2, 1)], vec![(1, 2), (2, 2)]],
            &[2, 2],
            [10.0, 6.0, 0.0],
        );
        assert_eq!(ledger.zones[0].air, [18.0, 6.0, 0.25]);
        assert_eq!(ledger.zones[1].air, [17.0, 7.0, 0.2]);
        assert!(ledger.valid());
    }

    fn fixture_building(world: &mut World, kind: BuildingType, x: i32, y: i32) -> Entity {
        world
            .spawn((
                Building {
                    kind,
                    is_provisional: false,
                },
                Transform::from_translation(WorldMap::grid_to_world(x, y).extend(0.0)),
            ))
            .id()
    }

    fn fixture_room(w: &mut World, x0: i32) -> (Entity, Vec<Entity>) {
        let tiles = vec![(x0 + 1, 21), (x0 + 2, 21), (x0 + 3, 21)];
        let room = w
            .spawn(Room {
                tiles: tiles.clone(),
                tile_signature: RoomTileSignature::from_tiles(&tiles),
                wall_tiles: vec![(x0, 21), (x0 + 4, 21)],
                door_tiles: vec![],
                bounds: RoomBounds::from_tile((x0 + 1, 21)),
                tile_count: 3,
            })
            .id();
        let mut vents = Vec::new();
        for x in x0..=x0 + 4 {
            for y in 20..=22 {
                let kind = if y == 21 && (x == x0 || x == x0 + 4) {
                    BuildingType::ScreamingVent
                } else if y != 21 || x == x0 || x == x0 + 4 {
                    BuildingType::Wall
                } else {
                    BuildingType::Floor
                };
                let e = fixture_building(w, kind, x, y);
                if kind == BuildingType::ScreamingVent {
                    vents.push(e);
                }
            }
        }
        (room, vents)
    }

    #[test]
    fn connected_fan_and_piped_intake_have_independent_operating_conditions() {
        use hw_energy::{PowerConsumer, PowerSupplyState};
        let mut w = World::new();
        w.init_resource::<RoomDetectionState>();
        let (room, _) = fixture_room(&mut w, 20);
        let (other, other_vents) = fixture_room(&mut w, 40);
        let unconnected = fixture_building(&mut w, BuildingType::ScreamingVent, 3, 3);
        let fan = fixture_building(
            &mut w,
            BuildingType::Equipment(Equipment::SupplyFan),
            21,
            21,
        );
        let intake = fixture_building(&mut w, BuildingType::Equipment(Equipment::Intake), 22, 24);
        for target in [fan, intake] {
            w.entity_mut(target)
                .insert((PowerConsumer { demand: 1.0 }, PowerSupplyState::Supplied));
        }
        fixture_building(&mut w, BuildingType::Equipment(Equipment::Basin), 22, 21);
        for y in 21..=24 {
            fixture_building(&mut w, BuildingType::Equipment(Equipment::Conduit), 22, y);
        }
        w.run_system_once(update_room_ventilation).unwrap();
        let step = w.register_system(advance);
        w.run_system_with(step, 0.25).unwrap();
        assert_eq!(
            *w.get::<super::super::VentReading>(unconnected).unwrap(),
            super::super::VentReading::Unconnected
        );
        let last_room = w
            .entity(other)
            .get_ref::<RoomVentilation>()
            .unwrap()
            .last_changed();
        let last_vents: Vec<_> = other_vents
            .iter()
            .map(|e| {
                w.entity(*e)
                    .get_ref::<super::super::VentReading>()
                    .unwrap()
                    .last_changed()
            })
            .collect();
        let ventilated = w.get::<RoomVentilation>(room).unwrap().0.supplied();
        assert!(w.get::<RoomClimate>(room).unwrap().sanctioned);
        w.entity_mut(fan).insert(PowerSupplyState::Disconnected);
        w.run_system_with(step, 0.0).unwrap();
        assert!(w.get::<RoomVentilation>(room).unwrap().0.supplied() < ventilated);
        assert!(w.get::<RoomClimate>(room).unwrap().water);
        assert_eq!(
            w.entity(other)
                .get_ref::<RoomVentilation>()
                .unwrap()
                .last_changed(),
            last_room
        );
        for (e, tick) in other_vents.iter().zip(last_vents) {
            assert_eq!(
                w.entity(*e)
                    .get_ref::<super::super::VentReading>()
                    .unwrap()
                    .last_changed(),
                tick
            );
        }
        w.entity_mut(intake).insert(PowerSupplyState::Disconnected);
        w.run_system_with(step, 0.0).unwrap();
        assert!(!w.get::<RoomClimate>(room).unwrap().sanctioned);
        w.run_system_with(step, 0.75).unwrap();
        let ledger = w.query::<&AirLedger>().single(&w).unwrap();
        assert_eq!((ledger.tick, ledger.remainder), (1, 0.0));
    }
}
