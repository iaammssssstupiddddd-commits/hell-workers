//! Reference-condition outdoor-air exchange in well-mixed rooms. All rates use game units.
mod balance;
pub mod network;
mod sparse;
use balance::Balance;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum VentilationState {
    Ventilated,
    Insufficient,
    NoOutdoorExchange,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Ventilation {
    required: f64,
    effective: f64,
    pub outdoor_in: f64,
    pub transfer_in: f64,
}

impl Ventilation {
    pub fn for_floor_tiles(tiles: usize) -> Self {
        Self {
            required: tiles as f64 / 20.0,
            effective: 0.0,
            outdoor_in: 0.0,
            transfer_in: 0.0,
        }
    }
    pub fn state(self) -> VentilationState {
        if self.effective == 0.0 {
            VentilationState::NoOutdoorExchange
        } else if self.effective + 1e-9 < self.required {
            VentilationState::Insufficient
        } else {
            VentilationState::Ventilated
        }
    }
    pub fn supplied(self) -> f64 {
        self.effective
    }
    pub fn required(self) -> f64 {
        self.required
    }
    pub fn deficit(self) -> f64 {
        (self.required - self.effective).max(0.0)
    }
}

/// Flow is positive from room `a` towards `b`. Outdoor pressures are
/// reference wind pressures on the exposed face, not an unlimited flow rate.
#[derive(Debug, Clone, Copy)]
pub enum AirBoundary {
    Outdoor { pressure: f64 },
    Room(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct Opening {
    pub a: usize,
    pub b: AirBoundary,
    pub conductance: f64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct OpeningFlow {
    pub net: f64,
    pub exchange_each_way: f64,
    pub connected_to_outdoors: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SolveCounts {
    pub room_groups: usize,
    pub largest_room_group: usize,
    pub matrix_solves: usize,
}

pub fn solve(floors: &[usize], openings: &[Opening]) -> (Vec<Ventilation>, Vec<OpeningFlow>) {
    let (rooms, flows, _) = solve_with_counts(floors, openings);
    (rooms, flows)
}

/// Outdoor pressure is a boundary at each grille. It never joins independent buildings.
pub fn solve_with_counts(
    floors: &[usize],
    openings: &[Opening],
) -> (Vec<Ventilation>, Vec<OpeningFlow>, SolveCounts) {
    let mut adjacent = vec![Vec::new(); floors.len()];
    for edge in openings {
        if let AirBoundary::Room(b) = edge.b {
            adjacent[edge.a].push(b);
            adjacent[b].push(edge.a);
        }
    }
    let mut membership = vec![usize::MAX; floors.len()];
    let mut groups = Vec::new();
    let mut stack = Vec::new();
    for root in 0..floors.len() {
        if membership[root] != usize::MAX {
            continue;
        }
        let group = groups.len();
        groups.push((Vec::new(), Vec::new()));
        membership[root] = group;
        stack.push(root);
        while let Some(a) = stack.pop() {
            for &b in &adjacent[a] {
                if membership[b] == usize::MAX {
                    membership[b] = group;
                    stack.push(b);
                }
            }
        }
    }
    let mut local = vec![0; floors.len()];
    for (i, &group) in membership.iter().enumerate() {
        local[i] = groups[group].0.len();
        groups[group].0.push(i);
    }
    for (i, edge) in openings.iter().enumerate() {
        groups[membership[edge.a]].1.push(i);
    }
    let mut rooms: Vec<_> = floors
        .iter()
        .map(|&f| Ventilation::for_floor_tiles(f))
        .collect();
    let mut flows = vec![OpeningFlow::default(); openings.len()];
    let mut counts = SolveCounts::default();
    for (zones, edges) in groups {
        let occupied = zones.iter().filter(|&&i| floors[i] != 0).count();
        counts.room_groups += usize::from(occupied != 0);
        counts.largest_room_group = counts.largest_room_group.max(occupied);
        let mut exterior_weight = 0.0;
        let mut weighted_pressure = 0.0;
        for &i in &edges {
            let edge = openings[i];
            if let AirBoundary::Outdoor { pressure } = edge.b {
                exterior_weight += edge.conductance;
                weighted_pressure += edge.conductance * pressure;
            }
        }
        if exterior_weight == 0.0 {
            for i in edges {
                let edge = openings[i];
                let AirBoundary::Room(b) = edge.b else {
                    unreachable!("exterior conductance must be positive")
                };
                let mixing = mixing_rate(edge);
                rooms[edge.a].transfer_in += mixing;
                rooms[b].transfer_in += mixing;
                flows[i].exchange_each_way = mixing;
            }
        } else if zones.len() == 1 {
            let room = &mut rooms[zones[0]];
            let indoor = weighted_pressure / exterior_weight;
            for i in edges {
                let edge = openings[i];
                let AirBoundary::Outdoor { pressure } = edge.b else {
                    unreachable!("self-loop in a single zone")
                };
                let net = edge.conductance * (indoor - pressure);
                let exchange_each_way = mixing_rate(edge);
                flows[i] = OpeningFlow {
                    net,
                    exchange_each_way,
                    connected_to_outdoors: true,
                };
                room.outdoor_in += exchange_each_way + (-net).max(0.0);
            }
            if room.required > 0.0 {
                room.effective = room.outdoor_in;
            }
        } else {
            let loads: Vec<_> = zones.iter().map(|&i| floors[i]).collect();
            let paths: Vec<_> = edges
                .iter()
                .map(|&i| {
                    let e = openings[i];
                    let b = match e.b {
                        AirBoundary::Room(b) => AirBoundary::Room(local[b]),
                        outside => outside,
                    };
                    Opening {
                        a: local[e.a],
                        b,
                        ..e
                    }
                })
                .collect();
            let (values, results) = solve_balances(&loads, &paths);
            for (i, v) in zones.into_iter().zip(values) {
                rooms[i] = v;
            }
            for (i, f) in edges.into_iter().zip(results) {
                flows[i] = f;
            }
            counts.matrix_solves += 2;
        }
    }
    (rooms, flows, counts)
}

fn mixing_rate(opening: Opening) -> f64 {
    0.20 * opening.conductance
}

fn solve_balances(floors: &[usize], openings: &[Opening]) -> (Vec<Ventilation>, Vec<OpeningFlow>) {
    let mut pressure = Balance::new(floors.len());
    for opening in openings {
        let g = opening.conductance;
        match opening.b {
            AirBoundary::Outdoor { pressure: p } => pressure.outside(opening.a, g, p),
            AirBoundary::Room(b) => pressure.link(opening.a, b, g, g),
        }
    }
    let pressures = pressure.solve();
    let mut rooms: Vec<_> = floors
        .iter()
        .map(|&n| Ventilation::for_floor_tiles(n))
        .collect();
    let mut dilution = Balance::new(floors.len());
    let mut flows = Vec::with_capacity(openings.len());
    for &Opening { a, b, conductance } in openings {
        let p_other = match b {
            AirBoundary::Outdoor { pressure } => Some(pressure),
            AirBoundary::Room(i) => pressures[i],
        };
        let net = pressures[a]
            .zip(p_other)
            .map_or(0.0, |(p, q)| conductance * (p - q));
        // Unresolved turbulence exchanges equal volumes each way even when mean pressure is equal.
        let mixing = mixing_rate(Opening { a, b, conductance });
        let into_a = mixing + (-net).max(0.0);
        let from_a = mixing + net.max(0.0);
        match b {
            AirBoundary::Outdoor { .. } => {
                rooms[a].outdoor_in += into_a;
                dilution.outside(a, into_a, 0.0);
            }
            AirBoundary::Room(i) => {
                rooms[a].transfer_in += into_a;
                rooms[i].transfer_in += from_a;
                dilution.link(a, i, into_a, from_a);
            }
        }
        flows.push(OpeningFlow {
            net,
            exchange_each_way: mixing,
            connected_to_outdoors: pressures[a].is_some(),
        });
    }
    // Unit design concentration above outdoors, with a reference load proportional to floor area.
    // Incoming transfer air carries the upstream concentration. Reusing it cannot create clean air.
    for (i, room) in rooms.iter().enumerate() {
        dilution.source(i, room.required);
    }
    for (room, level) in rooms.iter_mut().zip(dilution.solve()) {
        if let Some(level) = level
            && level > 0.0
        {
            room.effective = room.required / level;
        }
    }
    (rooms, flows)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn outside(a: usize, pressure: f64) -> Opening {
        Opening {
            a,
            b: AirBoundary::Outdoor { pressure },
            conductance: 1.0,
        }
    }
    fn between(a: usize, b: usize) -> Opening {
        Opening {
            a,
            b: AirBoundary::Room(b),
            conductance: 1.0,
        }
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8, "{a} vs {b}");
    }

    #[test]
    fn independent_groups_match_the_full_balance_in_input_order() {
        let candidates = [
            between(3, 0),
            outside(1, 1.0),
            between(0, 2),
            outside(2, -1.0),
            outside(3, 1.0),
            outside(1, -1.0),
            outside(4, 1.0),
            between(2, 3),
        ];
        let floors = [9, 12, 0, 6, 0, 4];
        for mask in 0..256 {
            let paths: Vec<_> = candidates
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, e)| *e)
                .collect();
            let (rooms, flows, counts) = solve_with_counts(&floors, &paths);
            let mut cached = network::AirNetwork::new(&floors, &floors, &paths, &[]);
            cached.update(1., &vec![0.; paths.len()], &[0.; 6]);
            for (a, b) in rooms.iter().zip(&cached.rates) {
                near(a.effective, b.effective);
                near(a.outdoor_in, b.outdoor_in);
                near(a.transfer_in, b.transfer_in);
                assert_eq!(a.state(), b.state());
            }
            for (a, b) in flows.iter().zip(&cached.flows) {
                near(a.net, b.net);
                near(a.exchange_each_way, b.exchange_each_way);
                assert_eq!(a.connected_to_outdoors, b.connected_to_outdoors);
            }
            let (expected, reference_flows) = solve_balances(&floors, &paths);
            for (a, b) in rooms.into_iter().zip(expected) {
                assert_eq!(a.state(), b.state());
                near(a.effective, b.effective);
                near(a.outdoor_in, b.outdoor_in);
                near(a.transfer_in, b.transfer_in);
            }
            for (a, b) in flows.into_iter().zip(reference_flows) {
                near(a.net, b.net);
                near(a.exchange_each_way, b.exchange_each_way);
                assert_eq!(a.connected_to_outdoors, b.connected_to_outdoors);
            }
            if mask == 0 {
                assert_eq!(counts.matrix_solves, 0);
            }
            if mask == 255 {
                assert_eq!(
                    (
                        counts.room_groups,
                        counts.largest_room_group,
                        counts.matrix_solves
                    ),
                    (3, 2, 2)
                );
            }
        }
    }

    #[test]
    fn sealed_loops_have_no_outdoor_air_even_when_other_rooms_are_ventilated() {
        let (rooms, flows) = solve(
            &[9, 12, 1, 9],
            &[between(0, 1), between(1, 2), between(2, 0), outside(3, 1.0)],
        );
        for room in &rooms[..3] {
            assert_eq!(room.state(), VentilationState::NoOutdoorExchange);
            near(room.supplied(), 0.0);
        }
        assert!(!flows[1].connected_to_outdoors);
        near(rooms[3].supplied(), 0.2);
    }

    #[test]
    fn single_sided_exchange_and_opposite_facades_have_distinct_balances() {
        let (single, flow) = solve(&[9], &[outside(0, 1.0)]);
        near(flow[0].net, 0.0);
        near(single[0].supplied(), 0.2);
        assert_eq!(single[0].state(), VentilationState::Insufficient);
        let (same, _) = solve(&[9], &[outside(0, 1.0), outside(0, 1.0)]);
        near(same[0].supplied(), 0.4);
        let (cross, flows) = solve(&[9], &[outside(0, 1.0), outside(0, -1.0)]);
        near(cross[0].supplied(), 1.4);
        near(flows[0].net + flows[1].net, 0.0);
        assert_eq!(cross[0].state(), VentilationState::Ventilated);
    }

    #[test]
    fn transfer_air_carries_the_reference_load_and_conserves_mass() {
        let openings = [outside(0, 1.0), between(0, 1), outside(1, -1.0)];
        let (rooms, flows) = solve(&[9, 12], &openings);
        near(flows[0].net, -2.0 / 3.0);
        near(flows[0].net + flows[1].net, 0.0);
        near(flows[2].net - flows[1].net, 0.0);
        let c0 = rooms[0].required() / rooms[0].supplied();
        let c1 = rooms[1].required() / rooms[1].supplied();
        near(
            c0 * (0.2 + flows[0].net.max(0.0)) + c1 * (0.2 + flows[2].net.max(0.0)),
            1.05,
        );
        assert!(rooms[1].supplied() < rooms[1].outdoor_in + rooms[1].transfer_in);
        let (blind, flow) = solve(&[9, 12], &openings[..2]);
        near(flow[1].net, 0.0);
        assert!(blind[1].supplied() > 0.0 && blind[1].supplied() < blind[0].supplied());
    }
}
