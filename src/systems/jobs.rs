use bevy::prelude::*;
use crate::constants::*;
use crate::assets::GameAssets;
use crate::entities::colonist::{Colonist, Destination};
use crate::systems::logistics::{ResourceItem, ClaimedBy, InStockpile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingType {
    Wall,
    Floor,
}

#[derive(Component)]
pub struct Building(pub BuildingType);

#[derive(Component)]
pub struct Blueprint {
    pub kind: BuildingType,
    pub progress: f32, // 0.0 to 1.0
}

#[derive(Component)]
pub struct CurrentJob(pub Option<Entity>);

pub fn job_assignment_system(
    mut q_colonists: Query<(Entity, &mut CurrentJob, &mut Destination), With<Colonist>>,
    q_blueprints: Query<Entity, With<Blueprint>>,
    q_items_unclaimed: Query<Entity, (With<ResourceItem>, Without<ClaimedBy>, Without<InStockpile>)>,
    q_items_all: Query<Entity, With<ResourceItem>>,
    q_transforms: Query<&Transform>,
    mut commands: Commands,
) {
    for (col_entity, mut job, mut dest) in q_colonists.iter_mut() {
        if job.0.is_none() {
            // 1. 建築優先
            if let Some(bp_entity) = q_blueprints.iter().next() {
                job.0 = Some(bp_entity);
                if let Ok(bp_transform) = q_transforms.get(bp_entity) {
                    let target = bp_transform.translation.truncate();
                    if dest.0 != target {
                        dest.0 = target;
                    }
                }
            } 
            // 2. 運搬
            else if let Some(item_entity) = q_items_unclaimed.iter().next() {
                job.0 = Some(item_entity);
                commands.entity(item_entity).insert(ClaimedBy(col_entity));
                if let Ok(item_transform) = q_transforms.get(item_entity) {
                    let target = item_transform.translation.truncate();
                    if dest.0 != target {
                        dest.0 = target;
                    }
                }
            }
        } else {
            // ジョブの有効性チェック
            let job_entity = job.0.unwrap();
            let job_valid = q_blueprints.get(job_entity).is_ok() || q_items_all.get(job_entity).is_ok();
            if !job_valid {
                job.0 = None;
            }
        }
    }
}

pub fn construction_work_system(
    time: Res<Time>,
    q_colonists: Query<(&Transform, &CurrentJob), With<Colonist>>,
    mut q_blueprints: Query<(&Transform, &mut Blueprint)>,
) {
    for (col_transform, job) in q_colonists.iter() {
        if let Some(job_entity) = job.0 {
            if let Ok((bp_transform, mut bp)) = q_blueprints.get_mut(job_entity) {
                let dist = col_transform.translation.truncate().distance(bp_transform.translation.truncate());
                if dist < TILE_SIZE * 0.5 {
                    bp.progress += time.delta_secs() * 0.2; // 5 seconds to build
                }
            }
        }
    }
}

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
