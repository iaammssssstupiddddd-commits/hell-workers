use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct ForestZone {
    pub min: (i32, i32),
    pub max: (i32, i32),
    pub initial_count: u32,
    pub tree_positions: Vec<(i32, i32)>,
}

impl ForestZone {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.min.0 && x <= self.max.0 && y >= self.min.1 && y <= self.max.1
    }
}

pub fn default_forest_zones() -> Vec<ForestZone> {
    vec![
        ForestZone {
            min: (15, 44),
            max: (32, 60),
            initial_count: 16,
            tree_positions: vec![
                (18, 48),
                (22, 53),
                (26, 47),
                (20, 56),
                (25, 50),
                (29, 55),
                (17, 52),
                (24, 58),
                (21, 45),
                (27, 51),
                (19, 54),
                (23, 46),
                (28, 54),
                (16, 49),
                (30, 48),
                (22, 57),
            ],
        },
        ForestZone {
            min: (14, 20),
            max: (30, 35),
            initial_count: 16,
            tree_positions: vec![
                (17, 24),
                (21, 29),
                (25, 23),
                (19, 32),
                (24, 26),
                (28, 31),
                (16, 28),
                (23, 34),
                (20, 21),
                (26, 27),
                (18, 30),
                (22, 22),
                (27, 30),
                (15, 25),
                (29, 24),
                (21, 33),
            ],
        },
        ForestZone {
            min: (9, 75),
            max: (43, 96),
            initial_count: 60,
            tree_positions: vec![
                (12, 78),
                (18, 83),
                (24, 77),
                (16, 88),
                (22, 82),
                (28, 87),
                (14, 81),
                (20, 94),
                (10, 85),
                (26, 80),
                (32, 85),
                (15, 90),
                (21, 76),
                (27, 91),
                (13, 86),
                (19, 79),
                (25, 93),
                (31, 78),
                (11, 82),
                (17, 95),
                (23, 84),
                (29, 89),
                (35, 80),
                (38, 86),
                (40, 82),
                (36, 92),
                (33, 88),
                (30, 76),
                (37, 79),
                (34, 94),
                (39, 88),
                (41, 84),
                (14, 93),
                (20, 87),
                (26, 77),
                (32, 91),
                (18, 81),
                (24, 95),
                (30, 83),
                (36, 77),
                (12, 89),
                (22, 86),
                (28, 78),
                (34, 84),
                (16, 76),
                (38, 90),
                (40, 78),
                (42, 88),
                (13, 84),
                (19, 92),
                (25, 79),
                (31, 93),
                (37, 83),
                (15, 77),
                (21, 89),
                (27, 82),
                (33, 77),
                (39, 93),
                (35, 87),
                (41, 81),
            ],
        },
    ]
}

pub fn find_regrowth_position(
    zone: &ForestZone,
    occupied_positions: &HashSet<(i32, i32)>,
    mut is_walkable: impl FnMut(i32, i32) -> bool,
) -> Option<(i32, i32)> {
    zone.tree_positions
        .iter()
        .copied()
        .find(|&(x, y)| !occupied_positions.contains(&(x, y)) && is_walkable(x, y))
}
