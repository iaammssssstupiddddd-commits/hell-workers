use super::components::{DreamParticle, DreamVisualState};
use super::dream_bubble_material::{DreamBubbleMaterial, DreamBubbleUiMaterial};
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

fn particle_color_for_quality(quality: DreamQuality) -> LinearRgba {
    match quality {
        DreamQuality::VividDream => LinearRgba::new(0.55, 0.8, 1.0, 1.0),
        DreamQuality::NormalDream => LinearRgba::new(0.55, 0.65, 0.95, 1.0),
        DreamQuality::NightTerror => LinearRgba::new(0.95, 0.45, 0.55, 1.0),
        DreamQuality::Awake => LinearRgba::WHITE,
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
    q_souls: Query<
        Entity,
        (
            With<DamnedSoul>,
            With<DreamState>,
            Without<DreamVisualState>,
        ),
    >,
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DreamBubbleMaterial>>,
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

        let mesh = meshes.add(Circle::new(0.5)); // 半径0.5の円（スケールでサイズ調整）
        let material = materials.add(DreamBubbleMaterial {
            color,
            time: 0.0,
            alpha: 0.85,
            mass: 1.0,
        });

        commands.spawn((
            DreamParticle {
                owner: soul_entity,
                quality: dream.quality,
                lifetime: particle_lifetime,
                max_lifetime: particle_lifetime,
                velocity,
                phase: rng.gen_range(0.0..=std::f32::consts::TAU),
            },
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(x_offset, y_offset, Z_VISUAL_EFFECT - Z_CHARACTER)
                .with_scale(Vec3::splat(size)),
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DreamBubbleMaterial>>,
    mut q_rest_areas: Query<(
        Entity,
        &Transform,
        &RestArea,
        Option<&RestAreaOccupants>,
        &mut DreamVisualState,
    )>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    q_ui_bubble_layer: Query<(Entity, &crate::interface::ui::components::UiMountSlot)>,
    ui_nodes: Res<crate::interface::ui::components::UiNodeRegistry>,
    q_ui_transform: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut ui_bubble_materials: ResMut<Assets<DreamBubbleUiMaterial>>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    let Some((camera, camera_transform)) = q_camera.iter().next() else {
        return;
    };
    let Some(dream_bubble_layer) = q_ui_bubble_layer
        .iter()
        .find(|(_, slot)| {
            matches!(
                slot,
                crate::interface::ui::components::UiMountSlot::DreamBubbleLayer
            )
        })
        .map(|(e, _)| e)
    else {
        return;
    };

    let viewport_size = camera
        .logical_viewport_size()
        .unwrap_or(Vec2::new(1920.0, 1080.0));

    let mut target_pos = Vec2::new(viewport_size.x - 80.0, 40.0);

    if let Some(entity) = ui_nodes.get_slot(crate::interface::ui::components::UiSlot::DreamPoolIcon) {
        if let Ok((computed, transform)) = q_ui_transform.get(entity) {
            let center = transform.translation * computed.inverse_scale_factor();
            target_pos = center;
        }
    }

    for (rest_area_entity, transform, rest_area, occupants_opt, mut visual_state) in q_rest_areas.iter_mut() {
        let occupant_count = occupants_opt
            .map(|occ| occ.len())
            .unwrap_or(0)
            .min(rest_area.capacity);

        if occupant_count == 0 {
            visual_state.particle_cooldown = 0.0;
            continue;
        }

        let scale_factor = (occupant_count as f32).sqrt().clamp(1.0, 3.0);
        let current_interval = crate::constants::DREAM_POPUP_INTERVAL;
        let max_particles = (DREAM_PARTICLE_MAX_PER_SOUL as f32 * scale_factor) as u8;

        if visual_state.particle_cooldown > 0.0 {
            visual_state.particle_cooldown = (visual_state.particle_cooldown - dt).max(0.0);
            continue;
        }

        if visual_state.active_particles >= max_particles {
            continue;
        }

        let particle_quality = DreamQuality::VividDream;
        let particle_lifetime = particle_lifetime_for_quality(particle_quality);
        let color = particle_color_for_quality(particle_quality);

        let velocity_y = rng.gen_range(12.0..=18.0) * (1.0 + (scale_factor - 1.0) * 0.5);
        let velocity = Vec2::new(rng.gen_range(-5.0..=5.0) * scale_factor, velocity_y);

        let x_offset = rng.gen_range(-DREAM_PARTICLE_SPAWN_OFFSET..=DREAM_PARTICLE_SPAWN_OFFSET) * scale_factor;
        let y_offset = DREAM_PARTICLE_SPAWN_OFFSET * 2.0 + rng.gen_range(0.0..=8.0);
        let size = rng.gen_range(DREAM_PARTICLE_SIZE_MIN..=DREAM_PARTICLE_SIZE_MAX) * (1.0 + (scale_factor - 1.0) * 0.5);

        let mesh = meshes.add(Circle::new(0.5));
        let material = materials.add(DreamBubbleMaterial {
            color,
            time: 0.0,
            alpha: 0.85,
            mass: 1.0,
        });

        commands.spawn((
            DreamParticle {
                owner: rest_area_entity,
                quality: particle_quality,
                lifetime: particle_lifetime,
                max_lifetime: particle_lifetime,
                velocity,
                phase: rng.gen_range(0.0..=std::f32::consts::TAU),
            },
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(x_offset, y_offset, Z_VISUAL_EFFECT - Z_CHARACTER)
                .with_scale(Vec3::splat(size)),
            ChildOf(rest_area_entity),
            Name::new("RestAreaDreamParticle"),
        ));

        let world_pos = transform.translation + Vec3::new(x_offset, y_offset, 0.0);
        if let Ok(start_pos) = camera.world_to_viewport(camera_transform, world_pos) {
            super::ui_particle::spawn_ui_particle(
                &mut commands,
                start_pos,
                target_pos,
                dream_bubble_layer,
                &mut ui_bubble_materials,
                0.5 * scale_factor,
            );
        }

        visual_state.active_particles = visual_state.active_particles.saturating_add(1);
        visual_state.particle_cooldown = current_interval;
    }
}

pub fn dream_particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_particles: Query<(Entity, &mut DreamParticle, &mut Transform, &MeshMaterial2d<DreamBubbleMaterial>)>,
    mut materials: ResMut<Assets<DreamBubbleMaterial>>,
    mut q_visual_state: Query<&mut DreamVisualState>,
) {
    let dt = time.delta_secs();

    for (entity, mut particle, mut transform, material_handle) in q_particles.iter_mut() {
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

        // マテリアルのtime・alphaを更新
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.time = time.elapsed_secs();
            material.alpha = life_ratio * 0.85;
        }
    }
}
