//! Sparse elimination for the two steady-state zone balances. Rows are stored as
//! ground coefficients plus positive coupling weights, avoiding subtraction of
//! nearly equal diagonals when a long row of rooms is reduced.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
struct Row {
    ground: f64,
    links: BTreeMap<usize, f64>,
    source: f64,
}

pub(super) struct Balance(Vec<Row>);

impl Balance {
    pub(super) fn new(zones: usize) -> Self {
        Self(vec![Row::default(); zones])
    }

    pub(super) fn outside(&mut self, zone: usize, weight: f64, reference: f64) {
        self.0[zone].ground += weight;
        self.0[zone].source += weight * reference;
    }

    pub(super) fn link(&mut self, a: usize, b: usize, into_a: f64, into_b: f64) {
        *self.0[a].links.entry(b).or_default() += into_a;
        *self.0[b].links.entry(a).or_default() += into_b;
    }

    pub(super) fn source(&mut self, zone: usize, amount: f64) {
        self.0[zone].source += amount;
    }

    pub(super) fn solve(mut self) -> Vec<Option<f64>> {
        let mut order = self
            .0
            .iter()
            .enumerate()
            .map(|(i, row)| (row.links.len(), i))
            .collect::<BTreeSet<_>>();
        let mut eliminated = Vec::with_capacity(order.len());
        while let Some((_, i)) = order.pop_first() {
            let row = std::mem::take(&mut self.0[i]);
            let diagonal = row.ground + row.links.values().sum::<f64>();
            for &j in row.links.keys() {
                let other = &mut self.0[j];
                order.remove(&(other.links.len(), j));
                let ratio = other.links.remove(&i).expect("paired opening") / diagonal;
                other.ground += ratio * row.ground;
                other.source += ratio * row.source;
                for (&k, &weight) in &row.links {
                    if k != j {
                        *other.links.entry(k).or_default() += ratio * weight;
                    }
                }
                order.insert((other.links.len(), j));
            }
            eliminated.push((i, row, diagonal));
        }
        let mut result = vec![None; eliminated.len()];
        for (i, row, diagonal) in eliminated.into_iter().rev() {
            // An isolated component has no outdoor reference. Propagate that
            // fact back through the component instead of inventing a pressure or dilution.
            if diagonal > 0.0 {
                let total = row.links.iter().try_fold(row.source, |sum, (&j, weight)| {
                    Some(sum + weight * result[j]?)
                });
                result[i] = total.map(|sum| sum / diagonal);
            }
        }
        result
    }
}
