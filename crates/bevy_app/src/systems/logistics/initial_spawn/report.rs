use bevy::prelude::*;

/// Initial spawn の集計結果。ログ出力を一元管理する。
pub struct InitialSpawnReport {
    pub trees_spawned: usize,
    pub rocks_spawned: usize,
    pub wood_spawned: usize,
    pub site_yard_spawned: bool,
    pub parking_spawned: bool,
    pub total_obstacles: usize,
    pub worldgen_seed: u64,
    pub used_fallback: bool,
}

impl InitialSpawnReport {
    pub fn log(&self) {
        info!(
            "SPAWNER: seed={} fallback={} Trees({}), Rocks({}), Wood({}) spawned. Site/Yard:{} Parking:{}. WorldMap obstacles:{}",
            self.worldgen_seed,
            self.used_fallback,
            self.trees_spawned,
            self.rocks_spawned,
            self.wood_spawned,
            self.site_yard_spawned,
            self.parking_spawned,
            self.total_obstacles,
        );
    }
}
