use bevy::prelude::*;
use crate::constants::*;
use crate::assets::GameAssets;
// use crate::entities::colonist::{Colonist, Destination};
// use crate::systems::logistics::{ResourceItem, ClaimedBy, InStockpile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingType {
    Wall,
    #[allow(dead_code)]
    Floor,
}

#[derive(Component)]
pub struct Building(#[allow(dead_code)] pub BuildingType);

#[derive(Component)]
pub struct Tree;

#[derive(Component)]
pub struct Rock;

#[derive(Component)]
pub struct Blueprint {
    pub kind: BuildingType,
    pub progress: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Component)]
#[allow(dead_code)]
pub struct CurrentJob(pub Option<Entity>);

pub fn building_completion_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
) {
    for (entity, bp, transform) in q_blueprints.iter_mut() {
        if bp.progress >= 1.0 {
            info!("BUILDING: Completed at {:?}", transform.translation);
            commands.entity(entity).despawn();
            commands.spawn((
                Building(bp.kind),
                Sprite {
                    image: game_assets.wall.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                *transform,
            ));
        }
    }
}
