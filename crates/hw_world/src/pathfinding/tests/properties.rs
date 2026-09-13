use super::super::{GridPos, PathGoalPolicy, PathWorld, PathfindingContext, find_path};
use proptest::prelude::*;
use std::collections::HashSet;

#[derive(Debug)]
struct SearchCase {
    width: i32,
    height: i32,
    cells: Vec<(bool, i32)>,
    start: GridPos,
    goal: GridPos,
}

impl PathWorld for SearchCase {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        ((0..self.width).contains(&x) && (0..self.height).contains(&y))
            .then_some((y * self.width + x) as usize)
    }

    fn idx_to_pos(&self, idx: usize) -> GridPos {
        (idx as i32 % self.width, idx as i32 / self.width)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.pos_to_idx(x, y).is_some_and(|i| self.cells[i].0)
    }

    fn get_door_cost(&self, x: i32, y: i32) -> i32 {
        self.cells[self.pos_to_idx(x, y).expect("in-bounds search cell")].1
    }
}

fn search_case() -> impl Strategy<Value = SearchCase> {
    (2_i32..=8, 2_i32..=8).prop_flat_map(|(width, height)| {
        let area = (width * height) as usize;
        (
            prop::collection::vec((any::<bool>(), 0_i32..=40), area),
            0..area,
            0..area,
        )
            .prop_map(move |(mut cells, start, goal)| {
                cells[start].0 = true;
                cells[goal].0 = true;
                SearchCase {
                    width,
                    height,
                    cells,
                    start: (start as i32 % width, start as i32 / width),
                    goal: (goal as i32 % width, goal as i32 / width),
                }
            })
    })
}

fn search(case: &SearchCase, context: &mut PathfindingContext) -> Option<Vec<GridPos>> {
    find_path(
        case,
        context,
        case.start,
        case.goal,
        PathGoalPolicy::RespectGoalWalkability,
    )
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, max_shrink_iters: 1024, ..ProptestConfig::default() })]

    #[test]
    fn properties_search_history_preserves_legal_paths(a in search_case(), b in search_case()) {
        let mut reused = PathfindingContext::default();
        for case in [&a, &b, &a] {
            let actual = search(case, &mut reused);
            prop_assert_eq!(&actual, &search(case, &mut PathfindingContext::default()));
            if let Some(path) = actual {
                prop_assert_eq!(path.first(), Some(&case.start));
                prop_assert_eq!(path.last(), Some(&case.goal));
                prop_assert_eq!(path.iter().collect::<HashSet<_>>().len(), path.len());
                for &(x, y) in &path {
                    prop_assert!(case.is_walkable(x, y));
                }
                for step in path.windows(2) {
                    let (from, to) = (step[0], step[1]);
                    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                    prop_assert!(dx.abs() <= 1 && dy.abs() <= 1 && (dx != 0 || dy != 0));
                    if dx != 0 && dy != 0 {
                        prop_assert!(case.is_walkable(from.0 + dx, from.1));
                        prop_assert!(case.is_walkable(from.0, from.1 + dy));
                    }
                }
            }
        }
    }
}
