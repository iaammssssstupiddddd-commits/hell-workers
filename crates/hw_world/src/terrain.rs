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

    pub fn priority(&self) -> u8 {
        match self {
            TerrainType::River => 0,
            TerrainType::Sand => 1,
            TerrainType::Dirt => 2,
            TerrainType::Grass => 3,
        }
    }
}
