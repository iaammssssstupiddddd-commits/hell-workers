use crate::pathfinding::PathWorld;
use rand::Rng;

pub fn find_nearby_walkable_grid(
    center: (i32, i32),
    world: &impl PathWorld,
    max_radius: i32,
) -> (i32, i32) {
    for dx in -max_radius..=max_radius {
        for dy in -max_radius..=max_radius {
            let test = (center.0 + dx, center.1 + dy);
            if world.is_walkable(test.0, test.1) {
                return test;
            }
        }
    }
    center
}

pub fn pick_random_walkable_grid_in_rect(
    world: &impl PathWorld,
    x_min: i32,
    x_max: i32,
    y_min: i32,
    y_max: i32,
    attempts: usize,
    rng: &mut impl Rng,
) -> Option<(i32, i32)> {
    for _ in 0..attempts {
        let x = rng.gen_range(x_min..=x_max);
        let y = rng.gen_range(y_min..=y_max);
        if world.is_walkable(x, y) {
            return Some((x, y));
        }
    }
    None
}
