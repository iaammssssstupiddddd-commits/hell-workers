mod post_process;
mod spawn;

use crate::assets::GameAssets;
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;

use super::Blueprint;

pub fn building_completion_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: WorldMapWrite,
    mut q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
    q_pending_bucket_storage: Query<
        (
            Entity,
            &crate::systems::logistics::PendingBelongsToBlueprint,
        ),
        With<crate::systems::logistics::BucketStorage>,
    >,
) {
    for (entity, bp, transform) in q_blueprints.iter_mut() {
        if !(bp.materials_complete() && bp.progress >= 1.0) {
            continue;
        }

        info!(
            "BUILDING: Completed at {:?} (materials: {:?})",
            transform.translation, bp.delivered_materials
        );
        commands.entity(entity).despawn();

        let building_entity =
            spawn::spawn_completed_building(&mut commands, bp, transform, &game_assets);

        let mut promoted_bucket_storage = Vec::new();
        if bp.kind == super::BuildingType::Tank {
            for (storage_entity, pending) in q_pending_bucket_storage.iter() {
                if pending.0 == entity {
                    commands
                        .entity(storage_entity)
                        .remove::<crate::systems::logistics::PendingBelongsToBlueprint>();
                    commands
                        .entity(storage_entity)
                        .insert(crate::systems::logistics::BelongsTo(building_entity));
                    promoted_bucket_storage.push(storage_entity);
                }
            }
        }

        // WorldMap 更新と ObstaclePosition 配置を hw_jobs の Observer に委譲
        commands.trigger(hw_jobs::BuildingCompletedEvent {
            building_entity,
            kind: bp.kind,
            occupied_grids: bp.occupied_grids.clone(),
        });

        post_process::apply_building_specific_post_process(
            &mut commands,
            entity,
            building_entity,
            bp,
            transform,
            &game_assets,
            &mut world_map,
            &promoted_bucket_storage,
        );
    }
}
