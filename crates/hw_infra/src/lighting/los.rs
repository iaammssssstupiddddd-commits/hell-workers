use super::{LightGridPos, LightOcclusionGrid};

pub fn has_line_of_sight(
    source: LightGridPos,
    target: LightGridPos,
    occlusion: &LightOcclusionGrid,
) -> bool {
    let dimensions = occlusion.dimensions();
    if !dimensions.contains(source) || !dimensions.contains(target) {
        return false;
    }
    if source == target {
        return true;
    }
    if occlusion.blocks_los_or_oob(target) {
        return false;
    }

    let delta_x = target.x - source.x;
    let delta_y = target.y - source.y;
    let count_x = delta_x.unsigned_abs();
    let count_y = delta_y.unsigned_abs();
    let step_x = delta_x.signum();
    let step_y = delta_y.signum();
    let mut traversed_x = 0_u32;
    let mut traversed_y = 0_u32;
    let mut current = source;

    while traversed_x < count_x || traversed_y < count_y {
        if traversed_x == count_x {
            current.y += step_y;
            traversed_y += 1;
            if occlusion.blocks_los_or_oob(current) {
                return false;
            }
            continue;
        }
        if traversed_y == count_y {
            current.x += step_x;
            traversed_x += 1;
            if occlusion.blocks_los_or_oob(current) {
                return false;
            }
            continue;
        }

        let cross_x = (u64::from(traversed_x) * 2 + 1) * u64::from(count_y);
        let cross_y = (u64::from(traversed_y) * 2 + 1) * u64::from(count_x);
        match cross_x.cmp(&cross_y) {
            std::cmp::Ordering::Less => {
                current.x += step_x;
                traversed_x += 1;
            }
            std::cmp::Ordering::Greater => {
                current.y += step_y;
                traversed_y += 1;
            }
            std::cmp::Ordering::Equal => {
                let side_x = LightGridPos::new(current.x + step_x, current.y);
                let side_y = LightGridPos::new(current.x, current.y + step_y);
                if occlusion.blocks_los_or_oob(side_x) || occlusion.blocks_los_or_oob(side_y) {
                    return false;
                }
                current.x += step_x;
                current.y += step_y;
                traversed_x += 1;
                traversed_y += 1;
            }
        }
        if occlusion.blocks_los_or_oob(current) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::{GridDimensions, OcclusionCell};

    fn grid_with_blocker(
        dimensions: GridDimensions,
        blocker: LightGridPos,
        cell: OcclusionCell,
    ) -> LightOcclusionGrid {
        let mut cells = vec![OcclusionCell::Clear; dimensions.area()];
        cells[dimensions.index(blocker).unwrap()] = cell;
        LightOcclusionGrid::from_cells(dimensions, cells).unwrap()
    }

    fn segment_touches_cell(
        source: LightGridPos,
        target: LightGridPos,
        cell: LightGridPos,
    ) -> bool {
        let source_x = f64::from(source.x) + 0.5;
        let source_y = f64::from(source.y) + 0.5;
        let delta_x = f64::from(target.x - source.x);
        let delta_y = f64::from(target.y - source.y);
        let mut enter = 0.0_f64;
        let mut leave = 1.0_f64;
        let bounds = [
            (-delta_x, source_x - f64::from(cell.x)),
            (delta_x, f64::from(cell.x + 1) - source_x),
            (-delta_y, source_y - f64::from(cell.y)),
            (delta_y, f64::from(cell.y + 1) - source_y),
        ];
        for (direction, distance) in bounds {
            if direction == 0.0 {
                if distance < 0.0 {
                    return false;
                }
                continue;
            }
            let ratio = distance / direction;
            if direction < 0.0 {
                enter = enter.max(ratio);
            } else {
                leave = leave.min(ratio);
            }
            if enter > leave {
                return false;
            }
        }
        true
    }

    #[test]
    fn corner_touch_checks_both_side_cells() {
        let dimensions = GridDimensions::new(3, 3).unwrap();
        let source = LightGridPos::new(0, 0);
        let target = LightGridPos::new(2, 2);
        for blocker in [LightGridPos::new(1, 0), LightGridPos::new(0, 1)] {
            let grid = grid_with_blocker(dimensions, blocker, OcclusionCell::CompletedWall);
            assert!(!has_line_of_sight(source, target, &grid));
        }
    }

    #[test]
    fn door_semantics_and_bounds_are_respected() {
        let dimensions = GridDimensions::new(4, 1).unwrap();
        let source = LightGridPos::new(0, 0);
        let target = LightGridPos::new(3, 0);
        let open = grid_with_blocker(dimensions, LightGridPos::new(2, 0), OcclusionCell::OpenDoor);
        let closed = grid_with_blocker(
            dimensions,
            LightGridPos::new(2, 0),
            OcclusionCell::ClosedDoor,
        );
        assert!(has_line_of_sight(source, target, &open));
        assert!(!has_line_of_sight(source, target, &closed));
        assert!(!has_line_of_sight(LightGridPos::new(-1, 0), target, &open));
    }

    #[test]
    fn seven_by_seven_single_blocker_matches_independent_geometry_reference() {
        let dimensions = GridDimensions::new(7, 7).unwrap();
        for source_y in 0..7 {
            for source_x in 0..7 {
                let source = LightGridPos::new(source_x, source_y);
                for target_y in 0..7 {
                    for target_x in 0..7 {
                        let target = LightGridPos::new(target_x, target_y);
                        for blocker_y in 0..7 {
                            for blocker_x in 0..7 {
                                let blocker = LightGridPos::new(blocker_x, blocker_y);
                                let grid = grid_with_blocker(
                                    dimensions,
                                    blocker,
                                    OcclusionCell::CompletedWall,
                                );
                                let expected = blocker == source
                                    || !segment_touches_cell(source, target, blocker);
                                assert_eq!(
                                    has_line_of_sight(source, target, &grid),
                                    expected,
                                    "source={source:?} target={target:?} blocker={blocker:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
