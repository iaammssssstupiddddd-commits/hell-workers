use super::components::{DreamParticle, DreamVisualState};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, DreamQuality, DreamState, GatheringBehavior, IdleBehavior, IdleState,
};
use crate::relationships::ParticipatingIn;
use bevy::prelude::ChildOf;
use bevy::prelude::*;
use rand::Rng;

fn particle_interval_for_quality(quality: DreamQuality) -> f32 {
    match quality {
        DreamQuality::VividDream => DREAM_PARTICLE_INTERVAL_VIVID,
        DreamQuality::NormalDream => DREAM_PARTICLE_INTERVAL_NORMAL,
        DreamQuality::NightTerror => DREAM_PARTICLE_INTERVAL_TERROR,
        DreamQuality::Awake => 0.0,
    }
}

fn particle_lifetime_for_quality(quality: DreamQuality) -> f32 {
    match quality {
        DreamQuality::VividDream => DREAM_PARTICLE_LIFETIME_VIVID,
        DreamQuality::NormalDream => DREAM_PARTICLE_LIFETIME_NORMAL,
        DreamQuality::NightTerror => DREAM_PARTICLE_LIFETIME_TERROR,
        DreamQuality::Awake => 0.0,
    }
}

fn particle_color_for_quality(quality: DreamQuality) -> Color {
    match quality {
        DreamQuality::VividDream => Color::srgb(0.55, 0.8, 1.0),
        DreamQuality::NormalDream => Color::srgb(0.55, 0.65, 0.95),
        DreamQuality::NightTerror => Color::srgb(0.95, 0.45, 0.55),
        DreamQuality::Awake => Color::WHITE,
    }
}

fn particle_sway_for_quality(quality: DreamQuality) -> f32 {
    match quality {
        DreamQuality::VividDream => DREAM_PARTICLE_SWAY_VIVID,
        DreamQuality::NightTerror => DREAM_PARTICLE_SWAY_TERROR,
        _ => 0.0,
    }
}

pub fn ensure_dream_visual_state_system(
    mut commands: Commands,
    q_souls: Query<Entity, (With<DamnedSoul>, With<DreamState>, Without<DreamVisualState>)>,
) {
    for entity in q_souls.iter() {
        commands.entity(entity).insert(DreamVisualState::default());
    }
}

pub fn dream_particle_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut q_souls: Query<
        (
            Entity,
            &IdleState,
            &DreamState,
            Option<&ParticipatingIn>,
            &mut DreamVisualState,
        ),
        With<DamnedSoul>,
    >,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (soul_entity, idle, dream, participating_in, mut visual_state) in q_souls.iter_mut() {
        visual_state.particle_cooldown = (visual_state.particle_cooldown - dt).max(0.0);
        let is_sleeping = idle.behavior == IdleBehavior::Sleeping
            || (idle.behavior == IdleBehavior::Gathering
                && idle.gathering_behavior == GatheringBehavior::Sleeping
                && participating_in.is_some());

        if !is_sleeping || dream.quality == DreamQuality::Awake {
            visual_state.particle_cooldown = 0.0;
            continue;
        }

        if visual_state.particle_cooldown > 0.0
            || visual_state.active_particles >= DREAM_PARTICLE_MAX_PER_SOUL
        {
            continue;
        }

        let particle_lifetime = particle_lifetime_for_quality(dream.quality);
        let color = particle_color_for_quality(dream.quality);
        let velocity = Vec2::new(rng.gen_range(-3.0..=3.0), rng.gen_range(12.0..=18.0));
        let x_offset = rng.gen_range(-DREAM_PARTICLE_SPAWN_OFFSET..=DREAM_PARTICLE_SPAWN_OFFSET);
        let y_offset = DREAM_PARTICLE_SPAWN_OFFSET + rng.gen_range(0.0..=4.0);
        let size = rng.gen_range(DREAM_PARTICLE_SIZE_MIN..=DREAM_PARTICLE_SIZE_MAX);

        commands.spawn((
            DreamParticle {
                owner: soul_entity,
                quality: dream.quality,
                lifetime: particle_lifetime,
                max_lifetime: particle_lifetime,
                velocity,
                phase: rng.gen_range(0.0..=std::f32::consts::TAU),
            },
            Sprite {
                image: assets.glow_circle.clone(),
                custom_size: Some(Vec2::splat(size)),
                color: color.with_alpha(0.85),
                ..default()
            },
            Transform::from_xyz(x_offset, y_offset, Z_VISUAL_EFFECT - Z_CHARACTER),
            ChildOf(soul_entity),
            Name::new("DreamParticle"),
        ));

        visual_state.active_particles = visual_state.active_particles.saturating_add(1);
        visual_state.particle_cooldown = particle_interval_for_quality(dream.quality);
    }
}

pub fn dream_particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_particles: Query<(Entity, &mut DreamParticle, &mut Transform, &mut Sprite)>,
    mut q_visual_state: Query<&mut DreamVisualState, With<DamnedSoul>>,
) {
    let dt = time.delta_secs();

    for (entity, mut particle, mut transform, mut sprite) in q_particles.iter_mut() {
        particle.lifetime -= dt;

        if particle.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            if let Ok(mut visual_state) = q_visual_state.get_mut(particle.owner) {
                visual_state.active_particles = visual_state.active_particles.saturating_sub(1);
            }
            continue;
        }

        particle.phase += dt * 8.0;
        let sway = particle.phase.sin() * particle_sway_for_quality(particle.quality) * dt;

        transform.translation.x += (particle.velocity.x * dt) + sway;
        transform.translation.y += particle.velocity.y * dt;

        let life_ratio = (particle.lifetime / particle.max_lifetime).clamp(0.0, 1.0);
        let base_color = particle_color_for_quality(particle.quality);
        sprite.color = base_color.with_alpha(life_ratio * 0.85);
    }
}
