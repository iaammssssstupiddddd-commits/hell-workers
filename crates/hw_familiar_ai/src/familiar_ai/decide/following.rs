use bevy::prelude::*;

use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use hw_jobs::AssignedTask;

type FollowingSoulsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static AssignedTask,
        &'static CommandedBy,
        &'static IdleState,
        &'static mut Destination,
        &'static mut Path,
    ),
    (With<DamnedSoul>, Without<Familiar>),
>;

/// 部下が使い魔を追尾するシステム
pub fn following_familiar_system(
    map: Res<hw_world::WorldMap>,
    mut q_souls: FollowingSoulsQuery,
    q_familiars: Query<(&Transform, &Familiar), With<Familiar>>,
) {
    for (_soul_entity, soul_transform, task, commanded_by, idle, mut dest, mut path) in
        q_souls.iter_mut()
    {
        if idle.behavior == IdleBehavior::ExhaustedGathering {
            continue;
        }
        if !matches!(task, AssignedTask::None) {
            continue;
        }

        if let Ok((fam_transform, familiar)) = q_familiars.get(commanded_by.0) {
            let fam_pos = fam_transform.translation.truncate();
            let soul_pos = soul_transform.translation.truncate();
            let command_radius = familiar.command_radius;

            let distance_sq = soul_pos.distance_squared(fam_pos);
            let radius_sq = command_radius * command_radius;

            if distance_sq > radius_sq {
                if dest.0.distance_squared(fam_pos) > 4.0 {
                    dest.0 = fam_pos;
                    path.waypoints.clear();
                    path.current_index = 0;
                    path.planned_destination = None;
                    path.validated_obstacle_version = 0;
                }
            } else {
                if map.clear_idle_doorway(soul_pos, &mut dest, &mut path) {
                    continue;
                }
                let has_active_path = path.current_index < path.waypoints.len();
                let destination_is_current = dest.0.distance_squared(soul_pos) <= f32::EPSILON;
                if has_active_path
                    || !destination_is_current
                    || path.planned_destination.is_some()
                    || path.validated_obstacle_version != 0
                {
                    dest.0 = soul_pos;
                    path.waypoints.clear();
                    path.current_index = 0;
                    path.planned_destination = None;
                    path.validated_obstacle_version = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::constants::TILE_SIZE;

    #[test]
    fn idle_soul_inside_command_radius_stops_at_its_current_position() {
        let mut app = App::new();
        app.init_resource::<hw_world::WorldMap>()
            .add_systems(Update, following_familiar_system);

        let familiar_position = Vec2::ZERO;
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                Transform::from_translation(familiar_position.extend(0.0)),
            ))
            .id();
        let soul_position = Vec2::new(TILE_SIZE * 2.0, 0.0);
        let soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                Transform::from_translation(soul_position.extend(0.0)),
                AssignedTask::None,
                CommandedBy(familiar),
                IdleState::default(),
                Destination(familiar_position),
                Path {
                    waypoints: vec![familiar_position],
                    current_index: 0,
                    planned_destination: Some(familiar_position),
                    validated_obstacle_version: 7,
                },
            ))
            .id();

        app.update();

        let destination = app.world().get::<Destination>(soul).unwrap();
        let path = app.world().get::<Path>(soul).unwrap();
        assert_eq!(destination.0, soul_position);
        assert!(path.waypoints.is_empty());
        assert_eq!(path.current_index, 0);
        assert_eq!(path.planned_destination, None);
        assert_eq!(path.validated_obstacle_version, 0);

        // A door completed underneath the stationary follower must wake it up, even inside command range.
        let (x, y) = hw_world::WorldMap::world_to_grid(soul_position);
        app.world_mut()
            .resource_mut::<hw_world::WorldMap>()
            .register_door(
                (x, y),
                Entity::PLACEHOLDER,
                hw_core::world::DoorState::Closed,
            );
        app.update();
        let exit = app.world().get::<Destination>(soul).unwrap().0;
        assert_ne!(hw_world::WorldMap::world_to_grid(exit), (x, y));
        app.update();
        assert_eq!(app.world().get::<Path>(soul).unwrap().waypoints, [exit]);
    }
}
