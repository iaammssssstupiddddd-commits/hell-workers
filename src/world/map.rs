use bevy::prelude::*;
use std::collections::HashMap;
use crate::constants::*;
use crate::assets::GameAssets;

#[derive(Component)]
pub struct Tile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Dirt,
    Stone,
}

impl TerrainType {
    pub fn is_walkable(&self) -> bool {
        match self {
            TerrainType::Grass | TerrainType::Dirt => true,
            TerrainType::Stone => false,
        }
    }
}

#[derive(Resource, Default)]
pub struct WorldMap {
    pub tiles: HashMap<(i32, i32), TerrainType>,
    pub buildings: HashMap<(i32, i32), Entity>,
    pub stockpiles: HashMap<(i32, i32), Entity>,
}

impl WorldMap {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { 
            tiles: HashMap::new(),
            buildings: HashMap::new(),
            stockpiles: HashMap::new(),
        }
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= MAP_WIDTH || y < 0 || y >= MAP_HEIGHT {
            return false;
        }
        self.tiles.get(&(x, y)).map_or(false, |t| t.is_walkable())
    }

    pub fn world_to_grid(pos: Vec2) -> (i32, i32) {
        let x = ((pos.x / TILE_SIZE) + (MAP_WIDTH as f32 / 2.0)).floor() as i32;
        let y = ((pos.y / TILE_SIZE) + (MAP_HEIGHT as f32 / 2.0)).floor() as i32;
        (x, y)
    }

    pub fn grid_to_world(x: i32, y: i32) -> Vec2 {
        Vec2::new(
            (x as f32 - MAP_WIDTH as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0,
            (y as f32 - MAP_HEIGHT as f32 / 2.0) * TILE_SIZE + TILE_SIZE / 2.0,
        )
    }
}

pub fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let (terrain, texture) = if (x + y) % 15 == 0 {
                (TerrainType::Stone, game_assets.stone.clone())
            } else if (x * y) % 5 == 0 {
                (TerrainType::Dirt, game_assets.dirt.clone())
            } else {
                (TerrainType::Grass, game_assets.grass.clone())
            };

            world_map.tiles.insert((x, y), terrain);

            commands.spawn((
                Tile,
                Sprite {
                    image: texture,
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(
                    (x as f32 - MAP_WIDTH as f32 / 2.0) * TILE_SIZE,
                    (y as f32 - MAP_HEIGHT as f32 / 2.0) * TILE_SIZE,
                    0.0,
                ),
            ));
        }
    }

    info!("BEVY_STARTUP: Map spawned ({}x{} tiles)", MAP_WIDTH, MAP_HEIGHT);
}
