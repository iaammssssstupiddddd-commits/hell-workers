//! Inputs, reference checks and trace encoding for the native climate benchmark.
use super::{AirLedger, AirRuntime, Fields, Grid, StageTimes};
use crate::systems::ventilation::{
    perf::{AirPerf, cpu_time_ns},
    replay::Trajectory,
    solver::Solver,
};

pub(super) fn set_occupancy(
    loads: &mut Vec<(usize, Grid, Fields)>,
    tick: u64,
    room_tiles: &[Vec<Grid>],
) {
    // Give both timed arms the same eight occupants at each HVAC tick.
    // Actor AI, movement and rendering still run in the actual world.
    loads.clear();
    for actor in 0..8 {
        let i = (actor * 31 + (tick / 12) as usize) % room_tiles.len();
        loads.push((i, room_tiles[i][0], [0.24, 0.04, 0.045]));
    }
}

impl AirPerf {
    pub(super) fn observe_step(
        &mut self,
        cost: u64,
        ledger: &AirLedger,
        cache: &AirRuntime,
        fresh: bool,
        integrated: bool,
    ) {
        self.climate_cpu_ns += cost;
        if fresh && ledger.zones.len() == 256 {
            self.climate_build_ns = cost;
        }
        let stages = cache.timing.as_ref().unwrap();
        self.climate_prepare_ns += stages.prepare;
        self.climate_step_ns += stages.step;
        self.climate_publish_ns += stages.publish;
        self.input_ns += stages.input;
        self.rooms_written += stages.rooms_written;
        self.vents_written += stages.vents_written;
        self.groups_changed += stages.groups_changed;
        self.publish_allocations += stages.publish_allocations;
        self.publish_bytes += stages.publish_bytes;
        self.wind = stages.wind;
        self.inspected_net = stages.inspected_net;
        self.synced_fans = stages.supplied;
        self.high_fans = stages.high_fans;
        self.exterior_net_abs = stages.exterior_net_abs;
        self.checked_steps += stages.checked;
        self.actual_mode = cache.factors.as_ref().unwrap().mode();
        self.climate_tick = ledger.tick;
        if self.pinned_vent.is_some() && integrated && (130..330).contains(&ledger.tick) {
            let (rates, flows) = cache.factors.as_ref().unwrap().readings();
            let trace = &mut self.publication_trace;
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
        self.climate_zones = ledger.zones.len();
    }
}

pub(super) fn measured_transport(
    solver: &mut Solver,
    tick: u64,
    outside: Fields,
    values: &mut [Fields],
    sources: &[Fields],
    timing: Option<&mut StageTimes>,
    tape: Option<&Trajectory>,
) {
    let start = timing.as_ref().map(|_| cpu_time_ns(false));
    let reference = tape.map(|t| t.frame(tick));
    let sources = reference.map(|(sources, _)| sources).unwrap_or(sources);
    solver.step(tick, outside, values, sources);
    if let Some((_, expected)) = reference {
        assert!(
            values
                .iter()
                .flatten()
                .zip(expected.iter().flatten())
                .all(|(&a, &b)| (a - b).abs() <= 1e-9 * (1. + b.abs())),
            "reference climate diverged at tick {tick}"
        );
    }
    if let (Some(t), Some(stages)) = (start, timing) {
        stages.checked += u64::from(reference.is_some());
        stages.step += cpu_time_ns(false) - t;
    }
}
