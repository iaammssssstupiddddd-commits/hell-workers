use super::WorldMap;

impl WorldMap {
    pub fn obstacle_count(&self) -> usize {
        self.obstacles.iter().filter(|&&blocked| blocked).count()
    }

    pub fn obstacle_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.obstacles
            .iter()
            .enumerate()
            .filter_map(|(idx, blocked)| blocked.then_some(idx))
    }

    pub fn add_obstacle(&mut self, x: i32, y: i32) {
        if let Some(idx) = self.pos_to_idx(x, y) {
            self.obstacles[idx] = true;
        }
    }

    pub fn remove_obstacle(&mut self, x: i32, y: i32) {
        if let Some(idx) = self.pos_to_idx(x, y) {
            self.obstacles[idx] = false;
        }
    }

    pub fn add_grid_obstacle(&mut self, grid: (i32, i32)) {
        self.add_obstacle(grid.0, grid.1);
    }

    pub fn remove_grid_obstacle(&mut self, grid: (i32, i32)) {
        self.remove_obstacle(grid.0, grid.1);
    }

    pub fn add_grid_obstacles<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            self.add_grid_obstacle(grid);
        }
    }

    pub fn reserve_building_footprint_tiles<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.add_grid_obstacles(grids);
    }
}
