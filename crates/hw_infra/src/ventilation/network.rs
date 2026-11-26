//! Fixed-topology room balances. Outdoor ports impose independent boundary
//! pressures. Only a transport component whose inputs changed is refactored.
use super::{
    AirBoundary, Opening, OpeningFlow, Ventilation, mixing_rate,
    sparse::{Factor, Matrix, Pattern},
};
use crate::air_transport::{Fields, Wall};
use std::{collections::HashMap, sync::Arc};

#[derive(Hash, PartialEq, Eq)]
struct Shape {
    zones: usize,
    links: Vec<(usize, usize)>,
}

/// Shared symbolic orders survive topology revisions. Only live shapes are retained.
#[derive(Default)]
pub struct PatternCache {
    shapes: HashMap<Shape, Arc<Pattern>>,
}

#[derive(Clone, Copy)]
enum Connection {
    Port(usize),
    Wall(f64),
}

#[derive(Clone, Copy)]
struct Edge {
    a: usize,
    b: Option<(usize, (usize, usize))>,
    kind: Connection,
}

struct Group {
    zones: Vec<usize>,
    edges: Vec<Edge>,
    pattern: Arc<Pattern>,
    capacity: Vec<f64>,
    outside: Vec<f64>,
    scratch: Vec<f64>,
}

fn groups(
    volumes: &[usize],
    ports: &[Opening],
    walls: &[Wall],
    heat: bool,
    cache: &mut PatternCache,
) -> Vec<Group> {
    let connections: Vec<_> = ports
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            let b = match p.b {
                AirBoundary::Room(b) => Some(b),
                AirBoundary::Outdoor { .. } => None,
            };
            (p.a, b, Connection::Port(idx))
        })
        .chain(walls.iter().map(|&(a, b, k)| (a, b, Connection::Wall(k))))
        .collect();
    let mut adjacency = vec![Vec::new(); volumes.len()];
    for &(a, neighbor, _) in &connections {
        let Some(b) = neighbor else { continue };
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    let mut membership = vec![usize::MAX; volumes.len()];
    let mut islands = Vec::new();
    let mut local = vec![0; volumes.len()];
    for root in 0..volumes.len() {
        if membership[root] != usize::MAX {
            continue;
        }
        let id = islands.len();
        membership[root] = id;
        let mut stack = vec![root];
        let mut zones = Vec::new();
        while let Some(i) = stack.pop() {
            zones.push(i);
            for &j in &adjacency[i] {
                if membership[j] == usize::MAX {
                    membership[j] = id;
                    stack.push(j);
                }
            }
        }
        zones.sort_unstable();
        for (i, &zone) in zones.iter().enumerate() {
            local[zone] = i;
        }
        islands.push((zones, Vec::new()));
    }
    for (a, b, kind) in connections {
        islands[membership[a]]
            .1
            .push((local[a], b.map(|g| local[g]), kind));
    }
    islands
        .into_iter()
        .map(|(zones, connections)| {
            let n = zones.len();
            let mut links: Vec<_> = connections
                .iter()
                .filter_map(|&(a, b, _)| b.map(|b| (a.min(b), a.max(b))))
                .collect();
            links.sort_unstable();
            links.dedup();
            let shape = Shape { zones: n, links };
            let pattern = Arc::clone(
                cache
                    .shapes
                    .entry(shape)
                    .or_insert_with_key(|s| Arc::new(Pattern::new(s.zones, &s.links))),
            );
            let edges = connections
                .into_iter()
                .map(|(a, b, kind)| Edge {
                    a,
                    b: b.map(|j| (j, pattern.indices(a, j))),
                    kind,
                })
                .collect();
            let capacity = zones
                .iter()
                .map(|&g| volumes[g] as f64 * if heat { 1.5 } else { 1.0 })
                .collect();
            Group {
                zones,
                edges,
                pattern,
                capacity,
                outside: vec![0.; n],
                scratch: vec![0.; n],
            }
        })
        .collect()
}

impl Group {
    fn transport(&mut self, flows: &[OpeningFlow]) -> Matrix {
        let mut m = self.pattern.matrix();
        self.outside.fill(0.);
        for edge in &self.edges {
            let (incoming, outgoing) = match edge.kind {
                Connection::Wall(k) => (k, k),
                Connection::Port(i) => {
                    let f = flows[i];
                    (
                        f.exchange_each_way + (-f.net).max(0.),
                        f.exchange_each_way + f.net.max(0.),
                    )
                }
            };
            if let Some((_, slots)) = edge.b {
                m.link(slots, incoming, outgoing);
            } else {
                self.outside[edge.a] += incoming;
            }
        }
        m.ground.copy_from_slice(&self.outside);
        m
    }

    fn step(
        &mut self,
        f: &Factor,
        field: usize,
        outside: Fields,
        values: &mut [Fields],
        sources: &[Fields],
    ) {
        for (i, &g) in self.zones.iter().enumerate() {
            self.scratch[i] = self.capacity[i] * values[g][field]
                + self.outside[i] * outside[field]
                + sources[g][field];
        }
        self.pattern.solve(f, &mut self.scratch);
        for (&g, &v) in self.zones.iter().zip(&self.scratch) {
            values[g][field] = v;
        }
    }
}

struct AirGroup {
    group: Group,
    pressure: Factor,
    exterior: bool,
    unit_flow: Vec<f64>,
    device_flow: Vec<f64>,
    drives: Vec<f64>,
    wind: Option<f64>,
    species: Option<Factor>,
    filter: Option<Factor>,
    removal: Vec<f64>,
}

impl AirGroup {
    fn new(group: Group, ports: &[Opening]) -> Self {
        let mut matrix = group.pattern.matrix();
        let mut exterior = false;
        for e in &group.edges {
            let Connection::Port(i) = e.kind else {
                unreachable!()
            };
            let g = ports[i].conductance;
            if let Some((_, slots)) = e.b {
                matrix.link(slots, g, g);
            } else {
                matrix.ground[e.a] += g;
                exterior = true;
            }
        }
        let pressure = group.pattern.factor(matrix);
        let count = group.edges.len();
        let mut this = Self {
            group,
            pressure,
            exterior,
            unit_flow: vec![0.; count],
            device_flow: vec![0.; count],
            drives: vec![0.; count],
            wind: None,
            species: None,
            filter: None,
            removal: Vec::new(),
        };
        if exterior {
            for (d, e) in this.drives.iter_mut().zip(&this.group.edges) {
                let Connection::Port(i) = e.kind else {
                    unreachable!()
                };
                if let AirBoundary::Outdoor { pressure } = ports[i].b {
                    *d = pressure;
                }
            }
            this.solve_pressure(ports);
            std::mem::swap(&mut this.unit_flow, &mut this.device_flow);
            this.drives.fill(0.);
        }
        this
    }

    fn solve_pressure(&mut self, ports: &[Opening]) {
        let g = &mut self.group;
        g.scratch.fill(0.);
        for (e, &p) in g.edges.iter().zip(&self.drives) {
            let Connection::Port(i) = e.kind else {
                unreachable!()
            };
            if e.b.is_none() {
                g.scratch[e.a] += ports[i].conductance * p;
            }
        }
        g.pattern.solve(&self.pressure, &mut g.scratch);
        for ((q, e), &drive) in self.device_flow.iter_mut().zip(&g.edges).zip(&self.drives) {
            let Connection::Port(i) = e.kind else {
                unreachable!()
            };
            let p_b = e.b.map_or(drive, |(b, _)| g.scratch[b]);
            *q = ports[i].conductance * (g.scratch[e.a] - p_b);
        }
    }

    fn update(
        &mut self,
        ports: &[Opening],
        wind: f64,
        drives: &[f64],
        removal: &[f64],
        rates: &mut [Ventilation],
        flows: &mut [OpeningFlow],
    ) -> bool {
        let mut device_changed = false;
        for (d, e) in self.drives.iter_mut().zip(&self.group.edges) {
            let Connection::Port(i) = e.kind else {
                unreachable!()
            };
            if e.b.is_none() && *d != drives[i] {
                *d = drives[i];
                device_changed = true;
            }
        }
        if device_changed {
            self.solve_pressure(ports);
        }
        let first = self.wind.is_none();
        let wind_changed = self.wind.replace(wind) != Some(wind);
        let mut changed = first;
        let g = &mut self.group;
        if first || device_changed || (self.exterior && wind_changed) {
            for (j, e) in g.edges.iter().enumerate() {
                let Connection::Port(i) = e.kind else {
                    unreachable!()
                };
                let q = self.unit_flow[j] * wind + self.device_flow[j];
                changed |= flows[i].net != q;
                flows[i] = OpeningFlow {
                    net: q,
                    exchange_each_way: mixing_rate(ports[i]),
                    connected_to_outdoors: self.exterior,
                };
            }
        }
        let filter_changed = self.removal.len() != g.zones.len()
            || g.zones
                .iter()
                .zip(&self.removal)
                .any(|(&z, &r)| removal[z] != r);
        if !changed && !filter_changed {
            return false;
        }
        let mut matrix = g.transport(flows);
        if changed {
            for (j, &z) in g.zones.iter().enumerate() {
                rates[z].outdoor_in = g.outside[j];
                rates[z].transfer_in = 0.;
                g.scratch[j] = rates[z].required;
            }
            for e in &g.edges {
                if let Some((b, _)) = e.b {
                    let Connection::Port(i) = e.kind else {
                        unreachable!()
                    };
                    let f = flows[i];
                    rates[g.zones[e.a]].transfer_in += f.exchange_each_way + (-f.net).max(0.);
                    rates[g.zones[b]].transfer_in += f.exchange_each_way + f.net.max(0.);
                }
            }
            if self.exterior {
                g.pattern
                    .solve(&g.pattern.factor(matrix.clone()), &mut g.scratch);
                for (&z, &level) in g.zones.iter().zip(&g.scratch) {
                    rates[z].effective = if level > 0. {
                        rates[z].required / level
                    } else {
                        0.
                    };
                }
            }
        }
        for (d, &c) in matrix.ground.iter_mut().zip(&g.capacity) {
            *d += c;
        }
        if changed {
            self.species = Some(g.pattern.factor(matrix.clone()));
        }
        if filter_changed {
            self.removal = g.zones.iter().map(|&z| removal[z]).collect();
        }
        self.filter = if self.removal.iter().any(|&r| r > 0.) {
            for (d, &r) in matrix.ground.iter_mut().zip(&self.removal) {
                *d += r;
            }
            Some(g.pattern.factor(matrix))
        } else {
            None
        };
        changed
    }
}

pub struct AirNetwork {
    ports: Vec<Opening>,
    air: Vec<AirGroup>,
    heat: Vec<(Group, Option<Factor>)>,
    air_by_zone: Vec<usize>,
    heat_by_zone: Vec<usize>,
    dirty_air: Vec<bool>,
    dirty_heat: Vec<bool>,
    last_wind: Option<f64>,
    last_drives: Vec<f64>,
    last_removal: Vec<f64>,
    changed_groups: Vec<usize>,
    pub rates: Vec<Ventilation>,
    pub flows: Vec<OpeningFlow>,
}

impl AirNetwork {
    pub fn new(floors: &[usize], volumes: &[usize], ports: &[Opening], walls: &[Wall]) -> Self {
        Self::with_patterns(floors, volumes, ports, walls, &mut PatternCache::default())
    }

    pub fn with_patterns(
        floors: &[usize],
        volumes: &[usize],
        ports: &[Opening],
        walls: &[Wall],
        cache: &mut PatternCache,
    ) -> Self {
        assert_eq!(floors.len(), volumes.len());
        let air: Vec<_> = groups(volumes, ports, &[], false, cache)
            .into_iter()
            .map(|g| AirGroup::new(g, ports))
            .collect();
        let heat: Vec<_> = groups(volumes, ports, walls, true, cache)
            .into_iter()
            .map(|g| (g, None))
            .collect();
        let mut air_by_zone = vec![0; floors.len()];
        let mut heat_by_zone = vec![0; floors.len()];
        for (index, a) in air.iter().enumerate() {
            for &zone in &a.group.zones {
                air_by_zone[zone] = index;
            }
        }
        for (index, (g, _)) in heat.iter().enumerate() {
            for &zone in &g.zones {
                heat_by_zone[zone] = index;
            }
        }
        let network = Self {
            ports: ports.to_vec(),
            air_by_zone,
            heat_by_zone,
            dirty_air: vec![false; air.len()],
            dirty_heat: vec![false; heat.len()],
            air,
            heat,
            last_wind: None,
            last_drives: vec![0.; ports.len()],
            last_removal: vec![0.; floors.len()],
            changed_groups: Vec::new(),
            rates: floors
                .iter()
                .map(|&n| Ventilation::for_floor_tiles(n))
                .collect(),
            flows: vec![OpeningFlow::default(); ports.len()],
        };
        cache.shapes.retain(|_, p| Arc::strong_count(p) > 1);
        network
    }

    /// Device pressures are additive offsets at the original exterior ports.
    pub fn update(&mut self, wind: f64, drives: &[f64], removal: &[f64]) -> bool {
        assert_eq!(drives.len(), self.ports.len());
        assert_eq!(removal.len(), self.rates.len());
        self.changed_groups.clear();
        let first = self.last_wind.is_none();
        let wind_changed = self.last_wind.replace(wind) != Some(wind);
        if !wind_changed && self.last_drives == drives && self.last_removal == removal {
            return false;
        }
        self.dirty_air.fill(first);
        self.dirty_heat.fill(first);
        for (i, (old, &new)) in self.last_drives.iter_mut().zip(drives).enumerate() {
            if *old != new {
                self.dirty_air[self.air_by_zone[self.ports[i].a]] = true;
                *old = new;
            }
        }
        for (i, (old, &new)) in self.last_removal.iter_mut().zip(removal).enumerate() {
            if *old != new {
                self.dirty_air[self.air_by_zone[i]] = true;
                *old = new;
            }
        }
        for (index, a) in self.air.iter_mut().enumerate() {
            if !(self.dirty_air[index] || wind_changed && a.exterior) {
                continue;
            }
            if a.update(
                &self.ports,
                wind,
                drives,
                removal,
                &mut self.rates,
                &mut self.flows,
            ) {
                self.changed_groups.push(index);
                // The heat graph contains every air edge, so this whole air group
                // belongs to one thermal component (possibly shared across walls).
                self.dirty_heat[self.heat_by_zone[a.group.zones[0]]] = true;
            }
        }
        for ((g, factor), &dirty) in self.heat.iter_mut().zip(&self.dirty_heat) {
            if dirty {
                let mut m = g.transport(&self.flows);
                for (v, &c) in m.ground.iter_mut().zip(&g.capacity) {
                    *v += c;
                }
                *factor = Some(g.pattern.factor(m));
            }
        }
        !self.changed_groups.is_empty()
    }

    /// Zone and opening indices use the same numbering as `rates` and `flows`.
    /// Every member of a changed air component can have a new dilution rate.
    pub fn changed_air_groups(
        &self,
    ) -> impl ExactSizeIterator<Item = (&[usize], impl Iterator<Item = usize>)> {
        self.changed_groups.iter().map(|&index| {
            let group = &self.air[index].group;
            let ports = group.edges.iter().map(|edge| match edge.kind {
                Connection::Port(i) => i,
                Connection::Wall(_) => unreachable!("air groups contain only openings"),
            });
            (group.zones.as_slice(), ports)
        })
    }

    pub fn step(&mut self, outside: Fields, values: &mut [Fields], sources: &[Fields]) {
        for (g, f) in &mut self.heat {
            g.step(f.as_ref().unwrap(), 0, outside, values, sources);
        }
        for a in &mut self.air {
            let species = a.species.as_ref().unwrap();
            a.group.step(species, 1, outside, values, sources);
            a.group.step(
                a.filter.as_ref().unwrap_or(species),
                2,
                outside,
                values,
                sources,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        air_transport::{Transport, weather},
        ventilation::solve,
    };

    fn near(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-9 * (1. + b.abs()), "{a} vs {b}");
    }

    #[test]
    fn grid_fill_in_matches_reference_under_reverse_wind_and_unequal_loads() {
        let n = 16;
        let floors = vec![4; n * n];
        let mut ports = Vec::new();
        for i in 0..n * n {
            if i % n > 0 {
                ports.push(Opening {
                    a: i,
                    b: AirBoundary::Room(i - 1),
                    conductance: 1.,
                });
            }
            if i >= n {
                ports.push(Opening {
                    a: i,
                    b: AirBoundary::Room(i - n),
                    conductance: 1.,
                });
            }
            if i % n == 0 || i % n == n - 1 {
                ports.push(Opening {
                    a: i,
                    b: AirBoundary::Outdoor {
                        pressure: if i % n == 0 { 1. } else { -1. },
                    },
                    conductance: 1.,
                });
            }
        }
        let mut cached = AirNetwork::new(&floors, &floors, &ports, &[]);
        let sources: Vec<_> = (0..n * n)
            .map(|i| {
                [
                    0.01 * i as f64,
                    0.03 * (i % 3) as f64,
                    0.001 * (i % 5) as f64,
                ]
            })
            .collect();
        let mut values = vec![[20., 8., 0.02]; n * n];
        let mut expected = values.clone();
        for tick in [0, 90, 180, 240, 360] {
            let (outside, wind) = weather(tick);
            let mut boundary = ports.clone();
            for p in &mut boundary {
                if let AirBoundary::Outdoor { pressure } = &mut p.b {
                    *pressure *= wind;
                }
            }
            let (rates, flows) = solve(&floors, &boundary);
            cached.update(wind, &vec![0.; ports.len()], &vec![0.; n * n]);
            for (r, s) in cached.rates.iter().zip(rates) {
                near(r.effective, s.effective);
                assert_eq!(r.state(), s.state());
            }
            for (a, b) in cached.flows.iter().zip(&flows) {
                near(a.net, b.net);
            }
            for field in 0..3 {
                Transport::prepare(&floors, &boundary, &flows, &[], field == 0).step(
                    field,
                    outside,
                    &mut expected,
                    &sources,
                );
            }
            cached.step(outside, &mut values, &sources);
            for (a, b) in values.iter().flatten().zip(expected.iter().flatten()) {
                near(*a, *b);
            }
        }
    }
    #[test]
    fn weather_power_filters_and_wall_only_neighbors_match_reference_each_tick() {
        let room = |a, b| Opening {
            a,
            b: AirBoundary::Room(b),
            conductance: 1.,
        };
        let external = |a, p| Opening {
            a,
            b: AirBoundary::Outdoor { pressure: p },
            conductance: 0.8,
        };
        let ports = [
            external(0, 1.),
            room(0, 1),
            room(1, 2),
            external(2, -1.),
            external(3, 1.),
            room(4, 5),
            external(6, 0.),
        ];
        let floors = [4, 0, 8, 4, 8, 4, 0];
        let volumes = [4, 0, 8, 4, 8, 4, 3];
        let walls = [(2, Some(3), 0.075), (4, None, 0.025)];
        let mut network = AirNetwork::new(&floors, &volumes, &ports, &walls);
        let mut values = vec![[22., 12., 0.2]; floors.len()];
        let mut expected = values.clone();
        for tick in 0..482 {
            let (outside, mut wind) = weather(tick);
            if tick == 481 {
                wind = 0.;
            }
            let mut drives = [0.; 7];
            if (40..230).contains(&tick) {
                drives[0] = 0.85 + 0.5;
            }
            let mut removal = [0.; 7];
            if (70..260).contains(&tick) {
                removal[2] = 0.65;
            }
            let mut sources = [[0.; 3]; 7];
            sources[0] = [0.2, 0.04, 0.045];
            sources[4] = [0.1, 0.15, 0.];
            let mut actual_ports = ports;
            for (p, d) in actual_ports.iter_mut().zip(drives) {
                if let AirBoundary::Outdoor { pressure } = &mut p.b {
                    *pressure = *pressure * wind + d;
                }
            }
            let (rates, flows) = solve(&floors, &actual_ports);
            network.update(wind, &drives, &removal);
            for (r, s) in network.rates.iter().zip(rates) {
                near(r.supplied(), s.supplied());
                near(r.outdoor_in, s.outdoor_in);
                near(r.transfer_in, s.transfer_in);
                assert_eq!(r.state(), s.state());
            }
            for (q, f) in network.flows.iter().zip(&flows) {
                near(q.net, f.net);
                near(q.exchange_each_way, f.exchange_each_way);
                assert_eq!(q.connected_to_outdoors, f.connected_to_outdoors);
            }
            for field in 0..3 {
                let model = Transport::with_removal(
                    &volumes,
                    &actual_ports,
                    &flows,
                    &walls,
                    field == 0,
                    if field == 2 { &removal } else { &[] },
                );
                model.step(field, outside, &mut expected, &sources);
            }
            network.step(outside, &mut values, &sources);
            for (a, b) in values.iter().flatten().zip(expected.iter().flatten()) {
                near(*a, *b);
            }
        }
    }
}
