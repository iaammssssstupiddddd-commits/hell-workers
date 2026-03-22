use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::gathering::{GATHERING_LEAVE_RADIUS, GatheringSpot};
use hw_core::relationships::ParticipatingIn;
use hw_core::soul::{
    DamnedSoul, DreamQuality, DreamState, GatheringBehavior, IdleBehavior, IdleState,
};
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};
use hw_spatial::{GatheringSpotSpatialGrid, SpatialGridOps};

type IdleVisualSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Sprite,
        &'static IdleState,
        &'static DamnedSoul,
        &'static SoulTaskVisualState,
        Option<&'static ParticipatingIn>,
        &'static DreamState,
    ),
>;

/// 怠惰行動のビジュアルフィードバック
pub fn idle_visual_system(
    q_spots: Query<&GatheringSpot>,
    spot_grid: Res<GatheringSpotSpatialGrid>,
    mut query: IdleVisualSoulQuery,
) {
    const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;

    for (mut transform, mut sprite, idle, soul, task_vs, participating_in, dream) in
        query.iter_mut()
    {
        if task_vs.phase != SoulTaskPhaseVisual::None {
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
            sprite.color = Color::WHITE;
            continue;
        }

        match idle.behavior {
            IdleBehavior::Sleeping => {
                transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                sprite.color = match dream.quality {
                    DreamQuality::VividDream => Color::srgba(0.5, 0.6, 0.9, 1.0),
                    DreamQuality::NightTerror => Color::srgba(0.8, 0.4, 0.4, 1.0),
                    _ => Color::srgba(0.6, 0.6, 0.7, 1.0),
                };
            }
            IdleBehavior::Resting => {
                transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                sprite.color = Color::srgba(0.6, 0.6, 0.7, 1.0);
            }
            IdleBehavior::Sitting => {
                transform.rotation = Quat::IDENTITY;
                transform.scale.y = 0.8;
                sprite.color = Color::srgba(0.8, 0.8, 0.8, 1.0);
            }
            IdleBehavior::Wandering | IdleBehavior::GoingToRest => {
                transform.rotation = Quat::IDENTITY;
                sprite.color = Color::WHITE;
            }
            IdleBehavior::Escaping => {
                transform.rotation = Quat::from_rotation_z(-0.1);
                sprite.color = Color::srgba(0.8, 0.9, 1.0, 1.0);
                let panic_pulse = (idle.total_idle_time * 8.0).sin() * 0.05 + 0.95;
                transform.scale = Vec3::splat(panic_pulse);
            }
            IdleBehavior::Drifting => {
                transform.rotation = Quat::IDENTITY;
                transform.scale = Vec3::ONE;
                sprite.color = Color::srgba(0.9, 0.9, 1.0, 0.85);
            }
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                let gathering_center = if let Some(p) = participating_in {
                    q_spots.get(p.0).ok().map(|s| s.center)
                } else {
                    let pos = transform.translation.truncate();
                    let nearby = spot_grid.get_nearby_in_radius(pos, GATHERING_LEAVE_RADIUS * 2.0);
                    nearby
                        .iter()
                        .filter_map(|&e| q_spots.get(e).ok())
                        .min_by(|a, b| {
                            a.center
                                .distance_squared(pos)
                                .partial_cmp(&b.center.distance_squared(pos))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|s| s.center)
                };

                if let Some(center) = gathering_center {
                    let current_pos = transform.translation.truncate();
                    let dist_from_center = (center - current_pos).length();
                    let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

                    if !has_arrived {
                        transform.rotation = Quat::IDENTITY;
                        transform.scale = Vec3::ONE;

                        if idle.behavior == IdleBehavior::ExhaustedGathering {
                            sprite.color = Color::srgba(0.7, 0.6, 0.8, 0.9);
                        } else {
                            sprite.color = Color::srgba(0.85, 0.75, 1.0, 0.85);
                        }
                    } else {
                        sprite.color = Color::srgba(0.8, 0.7, 1.0, 0.7);

                        match idle.gathering_behavior {
                            GatheringBehavior::Wandering => {
                                transform.rotation = Quat::IDENTITY;
                                let pulse_speed = 1.5;
                                let scale_offset =
                                    (idle.total_idle_time * pulse_speed).sin() * 0.05 + 1.0;
                                transform.scale = Vec3::splat(scale_offset);
                                sprite.color = Color::srgba(0.9, 0.8, 1.0, 0.85);
                            }
                            GatheringBehavior::Sleeping => {
                                transform.rotation =
                                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                                sprite.color = match dream.quality {
                                    DreamQuality::VividDream => Color::srgba(0.5, 0.5, 0.9, 0.7),
                                    DreamQuality::NightTerror => Color::srgba(0.8, 0.4, 0.5, 0.6),
                                    _ => Color::srgba(0.6, 0.5, 0.8, 0.6),
                                };
                                let breath = (idle.total_idle_time * 0.3).sin() * 0.02 + 0.95;
                                transform.scale = Vec3::splat(breath);
                            }
                            GatheringBehavior::Standing => {
                                transform.rotation = Quat::IDENTITY;
                                let breath = (idle.total_idle_time * 0.4).sin() * 0.03 + 1.0;
                                transform.scale = Vec3::splat(breath);
                            }
                            GatheringBehavior::Dancing => {
                                let sway_angle = (idle.total_idle_time * 5.0).sin() * 0.3;
                                transform.rotation = Quat::from_rotation_z(sway_angle);
                                let bounce = (idle.total_idle_time * 6.0).sin() * 0.15 + 1.0;
                                transform.scale = Vec3::new(1.0, bounce, 1.0);
                                sprite.color = Color::srgba(1.0, 0.7, 1.0, 1.0);
                            }
                        }
                    }
                } else {
                    transform.rotation = Quat::IDENTITY;
                    sprite.color = Color::WHITE;
                }
            }
        }

        if soul.motivation > 0.5
            && !matches!(
                idle.behavior,
                IdleBehavior::Gathering
                    | IdleBehavior::ExhaustedGathering
                    | IdleBehavior::Resting
                    | IdleBehavior::GoingToRest
                    | IdleBehavior::Drifting
            )
            && task_vs.phase == SoulTaskPhaseVisual::None
        {
            sprite.color = Color::srgb(1.0, 1.0, 0.8);
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
        }
    }
}
