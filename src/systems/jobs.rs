use bevy::prelude::*;
use crate::constants::*;
use crate::assets::GameAssets;

// --- Components ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingType {
    Wall,
    #[allow(dead_code)]
    Floor,
}

#[derive(Component)]
#[allow(dead_code)]
pub struct Building(pub BuildingType);

#[derive(Component)]
pub struct Tree;

#[derive(Component)]
pub struct Rock;

#[derive(Component)]
pub struct Blueprint {
    pub kind: BuildingType,
    pub progress: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkType {
    Chop,       // 伐採
    Mine,       // 採掘
    #[allow(dead_code)]
    Build,      // 建築
    Haul,       // 運搬
}

#[derive(Component)]
pub struct Designation {
    pub work_type: WorkType,
}

// --- Systems ---

pub fn building_completion_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
) {
    for (entity, bp, transform) in q_blueprints.iter_mut() {
        if bp.progress >= 1.0 {
            info!("BUILDING: Completed at {:?}", transform.translation);
            commands.entity(entity).despawn();
            
            let sprite_image = match bp.kind {
                BuildingType::Wall => game_assets.wall.clone(),
                BuildingType::Floor => game_assets.stone.clone(),
            };

            commands.spawn((
                Building(bp.kind),
                Sprite {
                    image: sprite_image,
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                *transform,
            ));
        }
    }
}
