//! Symbolic elimination is shared by all matrices on a component. Coefficients
//! are nonnegative incoming weights; a row's diagonal is their sum plus ground.
use std::collections::{BTreeSet, HashMap};

pub(super) struct Pattern {
    pivots: Vec<Pivot>,
    slots: usize,
    original: HashMap<(usize, usize), usize>,
}

struct Pivot {
    zone: usize,
    neighbors: Vec<(usize, usize, usize)>,
    scatter: Vec<usize>,
}

#[derive(Clone)]
pub(super) struct Matrix {
    pub ground: Vec<f64>,
    weights: Vec<f64>,
}

pub(super) struct Factor {
    inverse: Vec<f64>,
    weights: Vec<f64>,
}

impl Pattern {
    pub fn new(n: usize, edges: &[(usize, usize)]) -> Self {
        let mut adjacent = vec![BTreeSet::new(); n];
        for &(a, b) in edges {
            adjacent[a].insert(b);
            adjacent[b].insert(a);
        }
        let mut queue: BTreeSet<_> = adjacent
            .iter()
            .enumerate()
            .map(|(i, v)| (v.len(), i))
            .collect();
        let mut slots = HashMap::new();
        let mut slot = |i, j| {
            let next = slots.len();
            *slots.entry((i, j)).or_insert(next)
        };
        let mut original = HashMap::new();
        for &(a, b) in edges {
            original.insert((a, b), slot(a, b));
            original.insert((b, a), slot(b, a));
        }
        let mut pivots = Vec::with_capacity(n);
        while let Some((_, zone)) = queue.pop_first() {
            let remaining = std::mem::take(&mut adjacent[zone]);
            let mut neighbors = Vec::with_capacity(remaining.len());
            let mut scatter = Vec::new();
            for &j in &remaining {
                queue.remove(&(adjacent[j].len(), j));
                adjacent[j].remove(&zone);
                neighbors.push((j, slot(zone, j), slot(j, zone)));
                for &k in &remaining {
                    if j != k {
                        adjacent[j].insert(k);
                        scatter.push(slot(j, k));
                    }
                }
                queue.insert((adjacent[j].len(), j));
            }
            pivots.push(Pivot {
                zone,
                neighbors,
                scatter,
            });
        }
        Self {
            pivots,
            slots: slots.len(),
            original,
        }
    }

    pub fn indices(&self, a: usize, b: usize) -> (usize, usize) {
        (self.original[&(a, b)], self.original[&(b, a)])
    }

    pub fn matrix(&self) -> Matrix {
        Matrix {
            ground: vec![0.; self.pivots.len()],
            weights: vec![0.; self.slots],
        }
    }

    pub fn factor(&self, mut matrix: Matrix) -> Factor {
        let mut inverse = vec![0.; self.pivots.len()];
        let weights = &mut matrix.weights;
        for p in &self.pivots {
            let i = p.zone;
            let diagonal = matrix.ground[i]
                + p.neighbors
                    .iter()
                    .map(|&(_, out, _)| weights[out])
                    .sum::<f64>();
            // A floating component with no storage has no boundary forcing.
            if diagonal == 0. {
                continue;
            }
            inverse[i] = diagonal.recip();
            let mut scatter = p.scatter.iter();
            for &(j, _, into) in &p.neighbors {
                let forward = weights[into] * inverse[i];
                matrix.ground[j] += forward * matrix.ground[i];
                for &(k, out, _) in &p.neighbors {
                    if k != j {
                        weights[*scatter.next().unwrap()] += forward * weights[out];
                    }
                }
                weights[into] = forward;
            }
            for &(_, out, _) in &p.neighbors {
                weights[out] *= inverse[i];
            }
        }
        Factor {
            inverse,
            weights: matrix.weights,
        }
    }

    pub fn solve(&self, f: &Factor, rhs: &mut [f64]) {
        for p in &self.pivots {
            let value = rhs[p.zone];
            for &(j, _, into) in &p.neighbors {
                rhs[j] += f.weights[into] * value;
            }
        }
        for p in self.pivots.iter().rev() {
            let mut value = rhs[p.zone] * f.inverse[p.zone];
            for &(j, out, _) in &p.neighbors {
                value += f.weights[out] * rhs[j];
            }
            rhs[p.zone] = value;
        }
    }
}

impl Matrix {
    pub fn link(&mut self, (ab, ba): (usize, usize), into_a: f64, into_b: f64) {
        self.weights[ab] += into_a;
        self.weights[ba] += into_b;
    }
}
