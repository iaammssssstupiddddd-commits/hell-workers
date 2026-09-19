mod post_process;
pub(crate) mod spawn;

#[cfg(feature = "profiling")]
pub(crate) use spawn::{RenderPresentationClass, presentation_class};
pub(crate) use spawn::{
    attach_building_shell, spawn_building_3d_visual, structural_light_anchor_mesh_tag,
    structural_light_anchor_mesh_tag_with_direction,
};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildingCompletionSet;

use crate::assets::GameAssets;
use crate::plugins::startup::Building3dHandles;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::Blueprint;

pub fn building_completion_system(
    mut commands: Commands,
    q_blueprints: Query<(Entity, &Blueprint)>,
) {
    for (entity, bp) in &q_blueprints {
        if bp.materials_complete() && bp.progress >= 1.0 {
            commands.queue(move |world: &mut World| {
                commit_completed_building(world, entity);
            });
        }
    }
}

/// Finish map ownership, companions and ECS under one live validation boundary.
fn commit_completed_building(world: &mut World, entity: Entity) -> bool {
    let Some(bp) = world.get::<Blueprint>(entity) else {
        return false;
    };
    if !bp.materials_complete()
        || bp.progress < 1.0
        || bp.occupied_grids.is_empty()
        || world
            .get::<hw_jobs::BlueprintCancelRequested>(entity)
            .is_some()
        || world.get::<Transform>(entity).is_none()
        || world
            .resource::<WorldMap>()
            .validate_owned_footprint(entity, bp.occupied_grids.iter().copied())
            .is_err()
    {
        return false;
    }
    let occupied = bp.occupied_grids.clone();
    let promoted: Vec<_> = world
        .query_filtered::<(
            Entity,
            &crate::systems::logistics::PendingBelongsToBlueprint,
        ), With<crate::systems::logistics::BucketStorage>>()
        .iter(world)
        .filter(|(_, pending)| pending.0 == entity)
        .map(|(storage, _)| storage)
        .collect();
    let remaining_blockers: std::collections::HashSet<_> = world
        .query::<(&hw_jobs::ObstaclePosition, Option<&ChildOf>)>()
        .iter(world)
        .filter(|(_, parent)| !parent.is_some_and(|parent| parent.parent() == entity))
        .map(|(position, _)| (position.0, position.1))
        .filter(|grid| occupied.contains(grid))
        .collect();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    world.resource_scope(|world, mut map: Mut<WorldMap>| {
        let bp = world.get::<Blueprint>(entity).unwrap();
        let transform = world.get::<Transform>(entity).unwrap();
        let assets = world.resource::<GameAssets>();
        let handles = world.resource::<Building3dHandles>();
        let mut commands = Commands::new(&mut queue, world);
        let building =
            spawn::spawn_completed_building(&mut commands, bp, transform, assets, handles);
        map.complete_owned_building_footprint(
            entity,
            building,
            bp.kind,
            &occupied,
            &remaining_blockers,
        )
        .expect("ownership was validated at this exclusive commit boundary");
        let promoted = if bp.kind == super::BuildingType::Tank {
            &promoted[..]
        } else {
            &[]
        };
        for &storage in promoted {
            commands
                .entity(storage)
                .remove::<crate::systems::logistics::PendingBelongsToBlueprint>()
                .insert(crate::systems::logistics::BelongsTo(building));
        }
        post_process::apply_building_specific_post_process(
            &mut commands,
            post_process::PostProcessTargets {
                blueprint_entity: entity,
                building_entity: building,
            },
            bp,
            transform,
            assets,
            &mut map,
            promoted,
        );
        commands.entity(entity).despawn();
        hw_jobs::publish_building_completed(
            &mut commands,
            hw_jobs::BuildingCompletedEvent {
                blueprint_entity: entity,
                building_entity: building,
                kind: bp.kind,
                occupied_grids: occupied,
            },
        );
    });
    queue.apply(world);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_completion_rejects_one_conflicting_cell_before_spawning_or_promoting_test() {
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        let mut bp = Blueprint::new(hw_jobs::BuildingType::Tank, vec![(10, 10), (11, 10)]);
        bp.progress = 1.0;
        bp.delivered_materials = bp.required_materials.clone();
        let blueprint = world.spawn((bp, Transform::default())).id();
        let storage = world
            .spawn((
                crate::systems::logistics::BucketStorage,
                crate::systems::logistics::PendingBelongsToBlueprint(blueprint),
            ))
            .id();
        let other = world.spawn_empty().id();
        world
            .resource_mut::<WorldMap>()
            .set_building_occupancies(blueprint, [(10, 10), (11, 10)]);
        world
            .resource_mut::<WorldMap>()
            .set_building((11, 10), other);
        let version = world.resource::<WorldMap>().obstacle_version;
        assert!(!commit_completed_building(&mut world, blueprint));
        assert!(world.get::<Blueprint>(blueprint).is_some());
        assert!(
            world
                .get::<crate::systems::logistics::PendingBelongsToBlueprint>(storage)
                .is_some()
        );
        assert!(
            world
                .get::<crate::systems::logistics::BelongsTo>(storage)
                .is_none()
        );
        assert_eq!(
            world.resource::<WorldMap>().building_entity((10, 10)),
            Some(blueprint)
        );
        assert_eq!(
            world.resource::<WorldMap>().building_entity((11, 10)),
            Some(other)
        );
        assert_eq!(world.resource::<WorldMap>().obstacle_version, version);
        assert_eq!(world.query::<&hw_jobs::Building>().iter(&world).count(), 0);
    }
}
