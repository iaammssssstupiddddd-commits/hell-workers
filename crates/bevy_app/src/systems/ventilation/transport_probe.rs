//! Profiling-only, fixed-coefficient backward Euler experiment for the 2×2-room fixture.
use super::perf::cpu_time_ns;
use hw_infra::ventilation::{AirBoundary, Opening, OpeningFlow};
use serde::Serialize;
use std::collections::BTreeMap;

const MODES: [&str; 6] = [
    "airflow",
    "temperature",
    "humidity",
    "pollution",
    "humidity_pollution",
    "all",
];
const FIELDS: [&[usize]; 6] = [&[], &[0], &[1], &[2], &[1, 2], &[0, 1, 2]];
const INITIAL: [f64; 3] = [20.0, 7.0, 0.10];
const OUTDOOR: [f64; 3] = [12.0, 4.0, 0.02];
const WALL_CONDUCTANCE: f64 = 0.12;

#[derive(Clone, Copy, Serialize)]
struct Link {
    a: usize,
    b: Option<usize>,
    forward: f64,
    reverse: f64,
}

struct Pivot {
    reciprocal: f64,
    // LU multipliers for the forward and backward substitutions.
    links: Vec<(usize, f64, f64)>,
}

struct TransportMatrix {
    pivots: Vec<Pivot>,
    volume: Vec<f64>,
    outdoor_in: Vec<f64>,
}

impl TransportMatrix {
    fn build(floors: &[usize], links: &[Link], heat: bool) -> Self {
        let count = floors.len();
        let volume: Vec<_> = floors
            .iter()
            .map(|&v| v as f64 * if heat { 1.5 } else { 1.0 })
            .collect();
        let mut diagonal = volume.clone();
        let mut rows = vec![BTreeMap::<usize, f64>::new(); count];
        let mut outdoor_in = vec![0.0; count];
        let mut indoor_faces = vec![0; count];
        let wall = if heat { WALL_CONDUCTANCE } else { 0.0 };
        for &Link {
            a,
            b,
            forward,
            reverse,
        } in links
        {
            diagonal[a] += forward;
            if let Some(b) = b {
                indoor_faces[a] += 1;
                indoor_faces[b] += 1;
                diagonal[a] += wall;
                diagonal[b] += reverse + wall;
                *rows[a].entry(b).or_default() -= reverse + wall;
                *rows[b].entry(a).or_default() -= forward + wall;
            } else {
                outdoor_in[a] += reverse;
            }
        }
        for i in 0..count {
            // In this square-room fixture, each shared face has exactly one interior grille.
            assert!(indoor_faces[i] <= 4);
            let exterior = (4 - indoor_faces[i]) as f64 * wall;
            diagonal[i] += exterior;
            outdoor_in[i] += exterior;
        }
        let mut pivots = Vec::with_capacity(count);
        for k in 0..count {
            let row = std::mem::take(&mut rows[k]);
            assert!(diagonal[k].is_finite() && diagonal[k] > 0.0);
            let reciprocal = diagonal[k].recip();
            let mut factors = Vec::with_capacity(row.len());
            for (&j, &weight) in &row {
                let into_j = -rows[j].remove(&k).expect("paired airflow link") * reciprocal;
                for (&col, &coefficient) in &row {
                    if col == j {
                        diagonal[j] += into_j * coefficient;
                    } else {
                        *rows[j].entry(col).or_default() += into_j * coefficient;
                    }
                }
                factors.push((j, into_j, -weight * reciprocal));
            }
            pivots.push(Pivot {
                reciprocal,
                links: factors,
            });
        }
        Self {
            pivots,
            volume,
            outdoor_in,
        }
    }

    fn rhs(&self, field: usize) -> Vec<f64> {
        self.outdoor_in
            .iter()
            .enumerate()
            .map(|(room, &inflow)| {
                let scale = 1.0 + (room % 7) as f64 / 7.0;
                inflow * OUTDOOR[field] + [0.18, 0.04, 0.015][field] * scale
            })
            .collect()
    }

    fn advance(&self, values: &mut [f64], rhs: &[f64]) {
        for ((value, volume), input) in values.iter_mut().zip(&self.volume).zip(rhs) {
            *value = *value * volume + input;
        }
        for (i, pivot) in self.pivots.iter().enumerate() {
            let value = values[i];
            for &(j, forward, _) in &pivot.links {
                values[j] += forward * value;
            }
        }
        for (i, pivot) in self.pivots.iter().enumerate().rev() {
            let mut value = values[i] * pivot.reciprocal;
            for &(j, _, back) in &pivot.links {
                value += back * values[j];
            }
            values[i] = value;
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Observation {
    pub mode: &'static str,
    ticks: usize,
    cpu_ns: u64,
}

pub(crate) struct TransportProbe {
    matter: TransportMatrix,
    heat: TransportMatrix,
    inputs: [Vec<f64>; 3],
    states: Vec<Vec<Vec<f64>>>,
    links: Vec<Link>,
    tick: usize,
    setup_ns: u64,
}

impl TransportProbe {
    pub(super) fn new(floors: &[usize], openings: &[Opening], flows: &[OpeningFlow]) -> Self {
        let start = cpu_time_ns(false);
        assert_eq!(openings.len(), flows.len());
        assert!(
            floors.iter().all(|&v| v == 4),
            "dimension experiment needs 2×2 rooms"
        );
        let links: Vec<_> = openings
            .iter()
            .zip(flows)
            .map(|(opening, flow)| Link {
                a: opening.a,
                b: match opening.b {
                    AirBoundary::Room(b) => Some(b),
                    AirBoundary::Outdoor { .. } => None,
                },
                forward: flow.exchange_each_way + flow.net.max(0.0),
                reverse: flow.exchange_each_way + (-flow.net).max(0.0),
            })
            .collect();
        let matter = TransportMatrix::build(floors, &links, false);
        let heat = TransportMatrix::build(floors, &links, true);
        let inputs = [heat.rhs(0), matter.rhs(1), matter.rhs(2)];
        let states = FIELDS
            .iter()
            .map(|fields| {
                fields
                    .iter()
                    .map(|&field| vec![INITIAL[field]; floors.len()])
                    .collect()
            })
            .collect();
        Self {
            matter,
            heat,
            inputs,
            states,
            links,
            tick: 0,
            setup_ns: cpu_time_ns(false) - start,
        }
    }

    fn mode(tick: usize) -> usize {
        // Eight virtual seconds per block. Rotate the order on each pass through all modes.
        let block = tick / 8;
        (block + block / MODES.len()) % MODES.len()
    }

    pub(crate) fn observe(&mut self, virtual_secs: f64, limit: usize) -> Observation {
        let next = (virtual_secs.floor() as usize).min(limit);
        let mode = Self::mode(self.tick.min(limit - 1));
        let ticks = next - self.tick;
        // The engine caps one real frame at 250 ms, hence at most one virtual second at 4×.
        assert!(ticks <= 1, "multiple simulated seconds in a measured frame");
        let mut cpu_ns = 0;
        if ticks == 1 {
            let start = cpu_time_ns(false);
            for (&field, values) in FIELDS[mode].iter().zip(&mut self.states[mode]) {
                let matrix = if field == 0 { &self.heat } else { &self.matter };
                matrix.advance(values, &self.inputs[field]);
            }
            cpu_ns = cpu_time_ns(false) - start;
            self.tick = next;
        }
        Observation {
            mode: MODES[mode],
            ticks,
            cpu_ns,
        }
    }

    pub(crate) fn report(&self) -> serde_json::Value {
        let factor_links = [&self.heat, &self.matter].map(|matrix| {
            matrix
                .pivots
                .iter()
                .map(|pivot| pivot.links.len())
                .sum::<usize>()
        });
        serde_json::json!({
            "setup_ns":self.setup_ns,"ticks":self.tick,"modes":MODES,
            "links":self.links,"final_values":self.states,
            "factor_links":factor_links
        })
    }
}
