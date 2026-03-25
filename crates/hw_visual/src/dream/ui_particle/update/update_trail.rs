use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;
use hw_core::constants::*;

use super::super::super::components::DreamTrailGhost;
use super::super::super::dream_bubble_material::DreamBubbleUiMaterial;

pub(super) struct TrailGhostSpec {
    pub root: Entity,
    pub final_pos: Vec2,
    pub trail_size: f32,
    pub width_scale: f32,
    pub length_scale: f32,
    pub elapsed: f32,
    pub speed: f32,
    pub vel_dir: Vec2,
}

pub(super) fn spawn_trail_ghost(
    commands: &mut Commands,
    materials: &mut Assets<DreamBubbleUiMaterial>,
    spec: TrailGhostSpec,
) {
    let TrailGhostSpec { root, final_pos, trail_size, width_scale, length_scale, elapsed, speed, vel_dir } = spec;
    let mut trail_transform = Transform::from_translation(Vec3::ZERO);
    if speed > 1.0 {
        let angle = vel_dir.y.atan2(vel_dir.x) - std::f32::consts::FRAC_PI_2;
        trail_transform.rotation = Quat::from_rotation_z(angle);
    }

    let trail = commands
        .spawn((
            DreamTrailGhost {
                lifetime: DREAM_UI_TRAIL_LIFETIME,
                max_lifetime: DREAM_UI_TRAIL_LIFETIME,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(final_pos.x),
                top: Val::Px(final_pos.y),
                width: Val::Px(trail_size * width_scale),
                height: Val::Px(trail_size * length_scale),
                ..default()
            },
            trail_transform,
            MaterialNode(materials.add(DreamBubbleUiMaterial {
                color: LinearRgba::new(0.65, 0.9, 1.0, 1.0),
                time: elapsed,
                alpha: DREAM_UI_TRAIL_ALPHA,
                mass: 0.5,
                velocity_dir: vel_dir,
            })),
            GlobalZIndex(-1),
            Name::new("DreamTrailGhost"),
        ))
        .id();
    commands.entity(root).add_child(trail);
}
