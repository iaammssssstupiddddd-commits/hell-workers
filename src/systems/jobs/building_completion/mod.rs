mod post_process;
mod spawn;
mod world_update;

use crate::assets::GameAssets;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::Blueprint;

pub fn building_completion_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
    mut q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
    mut q_souls: Query<
        (&mut Transform, Entity),
        (
            With<crate::entities::damned_soul::DamnedSoul>,
            Without<super::Blueprint>,
        ),
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

        world_update::update_world_for_completed_building(
            &mut commands,
            building_entity,
            bp,
            &mut world_map,
            &mut q_souls,
        );

        post_process::apply_building_specific_post_process(
            &mut commands,
            building_entity,
            bp,
            transform,
            &game_assets,
            &mut world_map,
        );
    }
}
