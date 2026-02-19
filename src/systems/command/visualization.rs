use super::{DesignationIndicator, TaskMode};
use crate::constants::TILE_SIZE;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::game_state::TaskContext;
use crate::systems::jobs::{Designation, WorkType};
use bevy::prelude::*;

pub fn designation_visual_system(
    mut commands: Commands,
    q_designated: Query<
        (Entity, &Transform, &Designation, Option<&Visibility>),
        Changed<Designation>,
    >,
) {
    for (entity, transform, designation, visibility_opt) in q_designated.iter() {
        if visibility_opt == Some(&Visibility::Hidden) {
            continue;
        }

        let color = match designation.work_type {
            WorkType::Chop => Color::srgb(0.0, 1.0, 0.0),
            WorkType::Mine => Color::srgb(1.0, 0.0, 0.0),
            WorkType::Haul => Color::srgb(0.0, 0.0, 1.0),
            WorkType::HaulToMixer => Color::srgb(0.0, 0.0, 1.0), // Same as Haul
            WorkType::Build => Color::srgb(0.0, 0.5, 1.0),
            WorkType::GatherWater => Color::srgb(0.0, 0.8, 1.0), // Sky blue for water
            WorkType::CollectSand => Color::srgb(1.0, 0.8, 0.0), // Orange-yellow
            WorkType::Refine => Color::srgb(0.5, 0.0, 1.0),      // Purple
            WorkType::HaulWaterToMixer => Color::srgb(0.0, 0.8, 1.0), // Same as gather water
            WorkType::WheelbarrowHaul => Color::srgb(0.0, 0.0, 1.0), // Same as Haul
            WorkType::CollectBone => Color::srgb(0.9, 0.9, 0.8), // Bone white
            WorkType::ReinforceFloorTile => Color::srgb(0.0, 0.5, 1.0), // Same as Build
            WorkType::PourFloorTile => Color::srgb(0.0, 0.5, 1.0), // Same as Build
            WorkType::FrameWallTile => Color::srgb(0.0, 0.5, 1.0), // Same as Build
            WorkType::CoatWall => Color::srgb(0.0, 0.5, 1.0),    // Same as Build
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
    mut q_familiars: Query<(&ActiveCommand, &mut Sprite), With<Familiar>>,
) {
    for (command, mut sprite) in q_familiars.iter_mut() {
        if task_context.0 != TaskMode::None {
            sprite.color = Color::srgb(1.0, 1.0, 1.0);
            return;
        }

        match command.command {
            FamiliarCommand::Idle => {
                sprite.color = Color::srgb(0.6, 0.2, 0.2);
            }
            FamiliarCommand::GatherResources => {
                sprite.color = Color::srgb(1.0, 0.6, 0.2);
            }
            FamiliarCommand::Patrol => {
                sprite.color = Color::srgb(1.0, 0.3, 0.3);
            }
        }
    }
}
