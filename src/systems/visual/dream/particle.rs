use super::components::{DreamParticle, DreamVisualState};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, DreamQuality, DreamState, GatheringBehavior, IdleBehavior, IdleState,
};
use crate::relationships::{ParticipatingIn, RestAreaOccupants};
use crate::systems::jobs::RestArea;
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
    q_rest_areas: Query<Entity, (With<RestArea>, Without<DreamVisualState>)>,
) {
    for entity in q_souls.iter() {
        commands.entity(entity).insert(DreamVisualState::default());
    }
    for entity in q_rest_areas.iter() {
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

pub fn rest_area_dream_particle_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut q_rest_areas: Query<(
        Entity,
        &RestArea,
        Option<&RestAreaOccupants>,
        &mut DreamVisualState,
    )>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (rest_area_entity, rest_area, occupants_opt, mut visual_state) in q_rest_areas.iter_mut() {
        visual_state.particle_cooldown = (visual_state.particle_cooldown - dt).max(0.0);

        let occupant_count = occupants_opt
            .map(|occ| occ.len())
            .unwrap_or(0)
            .min(rest_area.capacity);

        if occupant_count == 0 {
            visual_state.particle_cooldown = 0.0;
            continue;
        }

        // 入居者数に応じたスケーリング係数（1名のときは1.0、増えると大きくなるが見た目の破綻を防ぐため最大を制限）
        let scale_factor = (occupant_count as f32).sqrt().clamp(1.0, 3.0);

        // パーティクル密度の増加（間隔を短くする）
        let base_interval = DREAM_PARTICLE_INTERVAL_VIVID; // 休憩所ボーナスとしてデフォルトでVivid相当の活発さとする
        let current_interval = base_interval / scale_factor;

        let max_particles = (DREAM_PARTICLE_MAX_PER_SOUL as f32 * scale_factor) as u8;

        if visual_state.particle_cooldown > 0.0 || visual_state.active_particles >= max_particles {
            continue;
        }

        // DreamQualityとしては（大量に集まっているイメージで）VividDreamとして視覚化する
        let particle_quality = DreamQuality::VividDream;
        let particle_lifetime = particle_lifetime_for_quality(particle_quality);
        let color = particle_color_for_quality(particle_quality);

        // 人数が増えるほど高く速く飛ぶようにする
        let velocity_y = rng.gen_range(12.0..=18.0) * (1.0 + (scale_factor - 1.0) * 0.5);
        let velocity = Vec2::new(rng.gen_range(-5.0..=5.0) * scale_factor, velocity_y);

        // スポーン範囲も建物のサイズや人数に合わせて広げる
        let x_offset = rng.gen_range(-DREAM_PARTICLE_SPAWN_OFFSET..=DREAM_PARTICLE_SPAWN_OFFSET)
            * scale_factor;
        // 建物から湧き出るように少し高めからスタート
        let y_offset = DREAM_PARTICLE_SPAWN_OFFSET * 2.0 + rng.gen_range(0.0..=8.0);

        // パーティクルのサイズもスケールさせる
        let size =
            rng.gen_range(DREAM_PARTICLE_SIZE_MIN..=DREAM_PARTICLE_SIZE_MAX) * (1.0 + (scale_factor - 1.0) * 0.5);

        commands.spawn((
            DreamParticle {
                owner: rest_area_entity,
                quality: particle_quality,
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
            ChildOf(rest_area_entity),
            Name::new("RestAreaDreamParticle"),
        ));

        visual_state.active_particles = visual_state.active_particles.saturating_add(1);
        visual_state.particle_cooldown = current_interval;
    }
}

pub fn dream_particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_particles: Query<(Entity, &mut DreamParticle, &mut Transform, &mut Sprite)>,
    mut q_visual_state: Query<&mut DreamVisualState>,
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
