use super::{DesignationIndicator, TaskMode};
use crate::app_contexts::TaskContext;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::systems::jobs::{Designation, WorkType};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;

type DesignatedVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Designation,
        Option<&'static Visibility>,
        Option<&'static hw_energy::SoulSpaTile>,
    ),
    Changed<Designation>,
>;

pub fn designation_visual_system(mut commands: Commands, q_designated: DesignatedVisualQuery) {
    for (entity, transform, designation, visibility_opt, spa_tile) in q_designated.iter() {
        if visibility_opt == Some(&Visibility::Hidden) {
            continue;
        }
        // Operational Spa slots already have their own occupancy/emission
        // presentation. Keep their GeneratePower jobs, without painting the
        // old golden task squares over the four-cell building artwork.
        if spa_tile.is_some() && designation.work_type == WorkType::GeneratePower {
            continue;
        }

        let color = match designation.work_type {
            WorkType::Chop => Color::srgb(0.0, 1.0, 0.0),
            WorkType::Mine => Color::srgb(1.0, 0.0, 0.0),
            WorkType::Haul => Color::srgb(0.0, 0.0, 1.0),
            WorkType::HaulToMixer => Color::srgb(0.0, 0.0, 1.0), // Same as Haul
            WorkType::Build => Color::srgb(0.0, 0.5, 1.0),
            WorkType::Move => Color::srgb(0.0, 0.7, 1.0),
            WorkType::GatherWater => Color::srgb(0.0, 0.8, 1.0), // Sky blue for water
            WorkType::Refine => Color::srgb(0.5, 0.0, 1.0),      // Purple
            WorkType::HaulWaterToMixer => Color::srgb(0.0, 0.8, 1.0), // Same as gather water
            WorkType::WheelbarrowHaul => Color::srgb(0.0, 0.0, 1.0), // Same as Haul
            WorkType::CollectBone => Color::srgb(0.9, 0.9, 0.8), // Bone white
            WorkType::ReinforceFloorTile => Color::srgb(0.0, 0.5, 1.0), // Same as Build
            WorkType::PourFloorTile => Color::srgb(0.0, 0.5, 1.0), // Same as Build
            WorkType::FrameWallTile => Color::srgb(0.0, 0.5, 1.0), // Same as Build
            WorkType::CoatWall => Color::srgb(0.0, 0.5, 1.0),    // Same as Build
            WorkType::GeneratePower => Color::srgb(1.0, 0.8, 0.0), // Golden yellow
            WorkType::Deconstruct => Color::srgb(1.0, 0.35, 0.1), // Ember orange
        };

        commands.spawn((
            DesignationIndicator(entity),
            Sprite {
                color: color.with_alpha(0.3),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.1)),
                ..default()
            },
            Transform::from_translation(transform.translation.truncate().extend(0.5)),
        ));
    }
}

pub fn familiar_command_visual_system(
    task_context: Res<TaskContext>,
    q_familiars: Query<&ActiveCommand, With<Familiar>>,
    mut q_visuals: Query<(&hw_visual::FamiliarVisualOwner, &mut Sprite)>,
) {
    for (owner, mut sprite) in q_visuals.iter_mut() {
        let Ok(command) = q_familiars.get(owner.owner) else {
            continue;
        };

        let desired_color = if task_context.0 != TaskMode::None {
            Color::srgb(1.0, 1.0, 1.0)
        } else {
            match command.command {
                FamiliarCommand::Idle => Color::srgb(0.6, 0.2, 0.2),
                FamiliarCommand::GatherResources => Color::srgb(1.0, 0.6, 0.2),
                FamiliarCommand::Patrol => Color::srgb(1.0, 0.3, 0.3),
            }
        };
        if sprite.color != desired_color {
            sprite.color = desired_color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spa_power_jobs_keep_designation_without_yellow_overlay() {
        let mut app = App::new();
        app.add_systems(Update, designation_visual_system);
        let site = app.world_mut().spawn_empty().id();
        let tile = app
            .world_mut()
            .spawn((
                hw_energy::SoulSpaTile {
                    parent_site: site,
                    grid_pos: (0, 0),
                },
                Designation {
                    work_type: WorkType::GeneratePower,
                },
                Transform::default(),
            ))
            .id();
        let other = app
            .world_mut()
            .spawn((
                Designation {
                    work_type: WorkType::GeneratePower,
                },
                Transform::default(),
            ))
            .id();
        app.update();
        let indicators: Vec<_> = app
            .world_mut()
            .query::<&DesignationIndicator>()
            .iter(app.world())
            .map(|indicator| indicator.0)
            .collect();
        assert_eq!(indicators, vec![other]);
        assert_eq!(
            app.world().get::<Designation>(tile).unwrap().work_type,
            WorkType::GeneratePower
        );
        // Re-activation and save/load reconstruction both insert Designation.
        app.world_mut().entity_mut(tile).insert(Designation {
            work_type: WorkType::GeneratePower,
        });
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&DesignationIndicator>()
                .iter(app.world())
                .count(),
            1
        );
    }
}
