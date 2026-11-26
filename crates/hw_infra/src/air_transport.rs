//! One virtual second of conservative implicit transport in well-mixed zones.
//! Factor once per flow network. Heat and a pair of passive species reuse the same geometry.
use crate::ventilation::{AirBoundary, Opening, OpeningFlow};
use std::collections::BTreeMap;

pub type Fields = [f64; 3]; // temperature, absolute humidity, miasma
pub type Wall = (usize, Option<usize>, f64);

struct Pivot {
    reciprocal: f64,
    links: Vec<(usize, f64, f64)>,
}

pub struct Transport {
    capacity: Vec<f64>,
    outside: Vec<f64>,
    pivots: Vec<Pivot>,
}

impl Transport {
    pub fn prepare(
        floors: &[usize],
        ports: &[Opening],
        flows: &[OpeningFlow],
        walls: &[Wall],
        heat: bool,
    ) -> Self {
        Self::with_removal(floors, ports, flows, walls, heat, &[])
    }

    pub fn with_removal(
        floors: &[usize],
        ports: &[Opening],
        flows: &[OpeningFlow],
        walls: &[Wall],
        heat: bool,
        removal: &[f64],
    ) -> Self {
        // Wall junctions have zero storage and are eliminated by their incident fluxes.
        let capacity: Vec<_> = floors
            .iter()
            .map(|&n| n as f64 * if heat { 1.5 } else { 1.0 })
            .collect();
        let mut diagonal = capacity.clone();
        for (value, sink) in diagonal.iter_mut().zip(removal) {
            *value += sink;
        }
        let mut rows = vec![BTreeMap::<usize, f64>::new(); floors.len()];
        let mut outside = vec![0.0; floors.len()];
        let mut link = |a: usize, b: Option<usize>, forward: f64, reverse: f64| {
            diagonal[a] += forward;
            if let Some(b) = b {
                diagonal[b] += reverse;
                *rows[a].entry(b).or_default() -= reverse;
                *rows[b].entry(a).or_default() -= forward;
            } else {
                outside[a] += reverse;
            }
        };
        for (port, flow) in ports.iter().zip(flows) {
            let b = match port.b {
                AirBoundary::Room(b) => Some(b),
                AirBoundary::Outdoor { .. } => None,
            };
            link(
                port.a,
                b,
                flow.exchange_each_way + flow.net.max(0.0),
                flow.exchange_each_way + (-flow.net).max(0.0),
            );
        }
        if heat {
            for &(a, b, rate) in walls {
                link(a, b, rate, rate);
            }
        }
        let mut pivots = Vec::with_capacity(rows.len());
        for i in 0..rows.len() {
            let row = std::mem::take(&mut rows[i]);
            let reciprocal = diagonal[i].recip();
            let mut links = Vec::with_capacity(row.len());
            for (&j, &weight) in &row {
                let forward =
                    -rows[j].remove(&i).expect("symmetric transport sparsity") * reciprocal;
                for (&k, &coefficient) in &row {
                    if k == j {
                        diagonal[j] += forward * coefficient;
                    } else {
                        *rows[j].entry(k).or_default() += forward * coefficient;
                    }
                }
                links.push((j, forward, -weight * reciprocal));
            }
            pivots.push(Pivot { reciprocal, links });
        }
        Self {
            capacity,
            outside,
            pivots,
        }
    }

    pub fn step(&self, field: usize, outside: Fields, values: &mut [Fields], sources: &[Fields]) {
        for (i, v) in values.iter_mut().enumerate() {
            v[field] =
                v[field] * self.capacity[i] + self.outside[i] * outside[field] + sources[i][field];
        }
        for (i, p) in self.pivots.iter().enumerate() {
            let value = values[i][field];
            for &(j, forward, _) in &p.links {
                values[j][field] += forward * value;
            }
        }
        for (i, p) in self.pivots.iter().enumerate().rev() {
            let mut x = values[i][field] * p.reciprocal;
            for &(j, _, back) in &p.links {
                x += back * values[j][field];
            }
            values[i][field] = x;
        }
    }
}

/// A repeatable draft weather cycle. The boundary changes once per 30 virtual seconds.
pub fn weather(tick: u64) -> (Fields, f64) {
    let phase = ((tick / 30) % 16) as f64 * std::f64::consts::TAU / 16.0;
    (
        [18.0 + 10.0 * phase.sin(), 8.0 + 2.5 * phase.cos(), 0.02],
        phase.cos(),
    )
}

pub fn saturation(temp: f64) -> f64 {
    let t = temp.clamp(-60.0, 70.0);
    216.7 * 6.112 * (17.62 * t / (243.12 + t)).exp() / (273.15 + t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sealed_transfer_conserves_each_species_and_heat() {
        let port = Opening {
            a: 0,
            b: AirBoundary::Room(1),
            conductance: 1.0,
        };
        let flow = OpeningFlow {
            net: 0.0,
            exchange_each_way: 0.4,
            connected_to_outdoors: false,
        };
        for d in 0..3 {
            let mut v = [[30.0; 3], [10.0; 3]];
            let model = Transport::prepare(&[2, 8], &[port], &[flow], &[(0, Some(1), 0.1)], d == 0);
            for _ in 0..20 {
                model.step(d, [0.0; 3], &mut v, &[[0.0; 3]; 2]);
            }
            assert!((2.0 * v[0][d] + 8.0 * v[1][d] - 140.0).abs() < 1e-8);
            assert!(v[0][d] < 30.0 && v[1][d] > 10.0);
        }
    }
}

#[cfg(test)]
mod junction_and_filter_tests {
    use super::*;
    #[test]
    fn zero_storage_junction_solves_instantaneously_and_filter_cleans_incoming_load() {
        let p = [
            Opening {
                a: 0,
                b: AirBoundary::Room(1),
                conductance: 1.0,
            },
            Opening {
                a: 1,
                b: AirBoundary::Outdoor { pressure: 0.0 },
                conductance: 1.0,
            },
        ];
        let q = [OpeningFlow {
            net: 0.0,
            exchange_each_way: 1.0,
            connected_to_outdoors: true,
        }; 2];
        let filtered = Transport::with_removal(&[1, 0], &p, &q, &[], false, &[0.5, 0.0]);
        let mut a = [[0.0; 3], [700.0; 3]];
        filtered.step(2, [1.0; 3], &mut a, &[[0.0; 3]; 2]);
        assert!((a[0][2] - 0.25).abs() < 1e-10);
        assert!((a[1][2] - 0.625).abs() < 1e-10);
    }
}
