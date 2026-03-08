use hw_core::constants::{Z_MAP, Z_MAP_DIRT, Z_MAP_GRASS, Z_MAP_SAND};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Dirt,
    River,
    Sand,
}

impl TerrainType {
    pub fn is_walkable(&self) -> bool {
        match self {
            TerrainType::Grass | TerrainType::Dirt | TerrainType::Sand => true,
            TerrainType::River => false,
        }
    }

    pub fn z_layer(&self) -> f32 {
        match self {
            TerrainType::River => Z_MAP,
            TerrainType::Sand => Z_MAP_SAND,
            TerrainType::Dirt => Z_MAP_DIRT,
            TerrainType::Grass => Z_MAP_GRASS,
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            TerrainType::River => 0,
            TerrainType::Sand => 1,
            TerrainType::Dirt => 2,
            TerrainType::Grass => 3,
        }
    }
}
