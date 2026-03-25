use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;
use hw_core::constants::*;
use rand::Rng;
use rand::rngs::ThreadRng;

use super::super::super::components::DreamGainUiParticle;
use super::super::super::dream_bubble_material::DreamBubbleUiMaterial;
use super::update_trail::spawn_trail_ghost;

pub(super) struct StandardParticleForces {
    to_target: Vec2,
    distance: f32,
    total_force: Vec2,
    effective_mass: f32,
    cluster_scale: f32,
}

pub(super) struct StandardParticleMotion {
    final_pos: Vec2,
    speed: f32,
    vel_dir: Vec2,
    width_scale: f32,
    length_scale: f32,
    visual_distance_ratio: f32,
}

pub(super) struct StandardInput {
    pub dt: f32,
    pub elapsed: f32,
    pub viewport_size: Vec2,
    pub current_pos: Vec2,
}

pub(super) struct ParticleState<'a> {
    pub particle: &'a mut DreamGainUiParticle,
    pub rng: &'a mut ThreadRng,
}

pub(super) struct NodeVisuals<'a> {
    pub node: &'a mut Node,
    pub mat_node: &'a MaterialNode<DreamBubbleUiMaterial>,
    pub transform: &'a mut Transform,
}

pub(super) struct TrailTimingCtx {
    pub dt: f32,
    pub elapsed: f32,
    pub ui_bubble_layer: Option<Entity>,
}

pub(super) fn update_standard_particle(
    input: StandardInput,
    state: ParticleState<'_>,
    visuals: NodeVisuals<'_>,
    materials: &mut Assets<DreamBubbleUiMaterial>,
    ui_bubble_layer: Option<Entity>,
    commands: &mut Commands<'_, '_>,
) -> bool {
    let StandardInput { dt, elapsed, viewport_size, current_pos } = input;
    let ParticleState { particle, rng } = state;
    let NodeVisuals { node, mat_node, transform } = visuals;

    let forces = compute_standard_particle_forces(dt, viewport_size, current_pos, particle, rng);

    let motion =
        integrate_standard_particle_motion(dt, viewport_size, current_pos, &forces, particle, node);

    update_standard_particle_visual(
        elapsed,
        particle,
        VisualData { mat_node, materials, transform, node },
        &motion,
        &forces,
    );

    if forces.distance < DREAM_UI_ARRIVAL_RADIUS {
        return true;
    }

    emit_standard_particle_trail(
        TrailTimingCtx { dt, elapsed, ui_bubble_layer },
        &motion,
        &forces,
        particle,
        commands,
        materials,
    );

    false
}

fn compute_standard_particle_forces(
    dt: f32,
    viewport_size: Vec2,
    current_pos: Vec2,
    particle: &mut DreamGainUiParticle,
    rng: &mut ThreadRng,
) -> StandardParticleForces {
    let buoyancy_ratio = (1.0 - (particle.time_alive / 1.5)).max(0.0);
    let buoyancy = Vec2::new(0.0, -DREAM_UI_BUOYANCY * buoyancy_ratio);

    let to_target = particle.target_pos - current_pos;
    let distance = to_target.length().max(1.0);

    let dist_ratio = (500.0 / distance.max(10.0)).clamp(1.0, 50.0);
    let distance_factor = 1.0 + (dist_ratio.ln() * 3.0).clamp(0.0, 15.0);
    let effective_mass = particle.mass + DREAM_UI_BASE_MASS_OFFSET;
    let attraction_strength = DREAM_UI_BASE_ATTRACTION * distance_factor * effective_mass;
    let mut attraction = to_target.normalize_or_zero() * attraction_strength;

    let size_dampening = 1.0 / effective_mass.sqrt().max(1.0);
    let vortex_ratio = (distance / 400.0).clamp(0.0, 1.0) * size_dampening;
    let tangent = Vec2::new(to_target.y, -to_target.x).normalize_or_zero();
    attraction += tangent * (attraction_strength * DREAM_UI_VORTEX_STRENGTH * vortex_ratio);

    particle.noise_timer -= dt;
    if particle.noise_timer <= 0.0 {
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        particle.noise_direction = Vec2::new(angle.cos(), angle.sin());
        particle.noise_timer = DREAM_UI_NOISE_INTERVAL;
    }

    let noise_ratio = (distance / 400.0).clamp(0.0, 1.0);
    let noise = particle.noise_direction * DREAM_UI_NOISE_STRENGTH * noise_ratio;

    let mut boundary = Vec2::ZERO;
    if current_pos.x < DREAM_UI_BOUNDARY_MARGIN {
        let ratio = 1.0 - (current_pos.x.max(0.0) / DREAM_UI_BOUNDARY_MARGIN).clamp(0.0, 1.0);
        boundary.x += DREAM_UI_BOUNDARY_PUSH * ratio;
    } else if current_pos.x > viewport_size.x - DREAM_UI_BOUNDARY_MARGIN {
        let ratio = 1.0
            - ((viewport_size.x - current_pos.x).max(0.0) / DREAM_UI_BOUNDARY_MARGIN)
                .clamp(0.0, 1.0);
        boundary.x -= DREAM_UI_BOUNDARY_PUSH * ratio;
    }
    if current_pos.y < DREAM_UI_BOUNDARY_MARGIN {
        let ratio = 1.0 - (current_pos.y.max(0.0) / DREAM_UI_BOUNDARY_MARGIN).clamp(0.0, 1.0);
        boundary.y += DREAM_UI_BOUNDARY_PUSH * ratio;
    } else if current_pos.y > viewport_size.y - DREAM_UI_BOUNDARY_MARGIN {
        let ratio = 1.0
            - ((viewport_size.y - current_pos.y).max(0.0) / DREAM_UI_BOUNDARY_MARGIN)
                .clamp(0.0, 1.0);
        boundary.y -= DREAM_UI_BOUNDARY_PUSH * ratio;
    }

    let total_force = buoyancy + attraction + noise + boundary;

    StandardParticleForces {
        to_target,
        distance,
        total_force,
        effective_mass,
        cluster_scale: merge_cluster_scale(particle.mass),
    }
}

fn integrate_standard_particle_motion(
    dt: f32,
    viewport_size: Vec2,
    current_pos: Vec2,
    forces: &StandardParticleForces,
    particle: &mut DreamGainUiParticle,
    node: &mut Node,
) -> StandardParticleMotion {
    particle.velocity += forces.total_force * dt;

    let mut drag = DREAM_UI_DRAG;
    if forces.distance < 50.0 {
        drag = drag.min(DREAM_UI_STRONG_DRAG);
    }
    let drag_factor = drag.powf(dt * 60.0);
    particle.velocity *= drag_factor;

    let min_speed = DREAM_UI_MIN_SPEED;
    let target_dir = forces.to_target.normalize_or_zero();
    let speed_toward_target = particle.velocity.dot(target_dir);
    if speed_toward_target < min_speed && forces.distance > 20.0 {
        let correction = target_dir * (min_speed - speed_toward_target.max(0.0));
        particle.velocity += correction;
    }

    let mut final_pos = current_pos + particle.velocity * dt;

    if final_pos.x < 0.0 {
        final_pos.x = 0.0;
        particle.velocity.x *= DREAM_UI_BOUNDARY_DAMPING;
    } else if final_pos.x > viewport_size.x {
        final_pos.x = viewport_size.x;
        particle.velocity.x *= DREAM_UI_BOUNDARY_DAMPING;
    }

    if final_pos.y < 0.0 {
        final_pos.y = 0.0;
        particle.velocity.y *= DREAM_UI_BOUNDARY_DAMPING;
    } else if final_pos.y > viewport_size.y {
        final_pos.y = viewport_size.y;
        particle.velocity.y *= DREAM_UI_BOUNDARY_DAMPING;
    }

    if final_pos.x < -DREAM_UI_FAILSAFE_MARGIN {
        final_pos.x = viewport_size.x;
    } else if final_pos.x > viewport_size.x + DREAM_UI_FAILSAFE_MARGIN {
        final_pos.x = 0.0;
    }

    if final_pos.y < -DREAM_UI_FAILSAFE_MARGIN {
        final_pos.y = viewport_size.y;
    } else if final_pos.y > viewport_size.y + DREAM_UI_FAILSAFE_MARGIN {
        final_pos.y = 0.0;
    }

    particle.prev_pos = final_pos;
    node.left = Val::Px(final_pos.x);
    node.top = Val::Px(final_pos.y);

    let speed = particle.velocity.length();
    let squash_stretch_ratio =
        (speed / DREAM_UI_SQUASH_MAX_SPEED).clamp(0.0, DREAM_UI_SQUASH_MAX_RATIO);
    let length_scale = 1.0 + squash_stretch_ratio;
    let width_scale = 1.0 / (1.0 + squash_stretch_ratio * 0.5);

    let visual_distance_ratio = {
        let start_dist = particle.start_pos.distance(particle.target_pos).max(1.0);
        (forces.distance / start_dist.max(100.0)).clamp(0.01, 1.0)
    };

    StandardParticleMotion {
        final_pos,
        speed,
        vel_dir: if speed > 1.0 {
            particle.velocity / speed
        } else {
            Vec2::ZERO
        },
        width_scale,
        length_scale,
        visual_distance_ratio,
    }
}

struct VisualData<'a> {
    mat_node: &'a MaterialNode<DreamBubbleUiMaterial>,
    materials: &'a mut Assets<DreamBubbleUiMaterial>,
    transform: &'a mut Transform,
    node: &'a mut Node,
}

fn update_standard_particle_visual(
    elapsed: f32,
    particle: &DreamGainUiParticle,
    vis: VisualData<'_>,
    motion: &StandardParticleMotion,
    forces: &StandardParticleForces,
) {
    let VisualData { mat_node, materials, transform, node } = vis;
    let shrink = (motion.visual_distance_ratio * 9.0 + 1.0).log10().max(0.1);

    node.width = Val::Px(
        DREAM_UI_PARTICLE_SIZE
            * forces.effective_mass.sqrt()
            * forces.cluster_scale
            * shrink
            * motion.width_scale,
    );
    node.height = Val::Px(
        DREAM_UI_PARTICLE_SIZE
            * forces.effective_mass.sqrt()
            * forces.cluster_scale
            * shrink
            * motion.length_scale,
    );

    if motion.speed > 1.0 {
        let angle = motion.vel_dir.y.atan2(motion.vel_dir.x) - std::f32::consts::FRAC_PI_2;
        transform.rotation = Quat::from_rotation_z(angle);
    }

    let base_color = if motion.visual_distance_ratio < 0.3 {
        let white_t = 1.0 - (motion.visual_distance_ratio / 0.3);
        let r = 0.65 + white_t * 0.35;
        let g = 0.9 + white_t * 0.1;
        LinearRgba::new(r, g, 1.0, 1.0)
    } else {
        LinearRgba::new(0.65, 0.9, 1.0, 1.0)
    };

    let alpha = if particle.time_alive < 0.2 {
        (particle.time_alive / 0.2).clamp(0.0, 1.0) * 0.9
    } else {
        0.9
    };

    if let Some(mat) = materials.get_mut(&mat_node.0) {
        mat.color = base_color;
        mat.alpha = alpha;
        mat.time = elapsed;
        mat.mass = particle.mass;
        mat.velocity_dir = motion.vel_dir;
    }
}

fn emit_standard_particle_trail(
    timing: TrailTimingCtx,
    motion: &StandardParticleMotion,
    forces: &StandardParticleForces,
    particle: &mut DreamGainUiParticle,
    commands: &mut Commands<'_, '_>,
    materials: &mut Assets<DreamBubbleUiMaterial>,
) {
    let TrailTimingCtx { dt, elapsed, ui_bubble_layer } = timing;
    particle.trail_cooldown -= dt;
    if particle.trail_cooldown <= 0.0 && motion.visual_distance_ratio > 0.15 {
        particle.trail_cooldown = DREAM_UI_TRAIL_INTERVAL;
        let trail_size = DREAM_UI_PARTICLE_SIZE
            * forces.effective_mass.sqrt()
            * forces.cluster_scale
            * (motion.visual_distance_ratio * 9.0 + 1.0).log10().max(0.1)
            * DREAM_UI_TRAIL_SIZE_RATIO;
        if let Some(root) = ui_bubble_layer {
            spawn_trail_ghost(
                commands,
                materials,
                super::update_trail::TrailGhostSpec {
                    root,
                    final_pos: motion.final_pos,
                    trail_size,
                    width_scale: motion.width_scale,
                    length_scale: motion.length_scale,
                    elapsed,
                    speed: motion.speed,
                    vel_dir: motion.vel_dir,
                },
            );
        }
    }
}

fn merge_cluster_scale(mass: f32) -> f32 {
    if mass >= 6.0 {
        1.20
    } else if mass >= 3.0 {
        1.25
    } else {
        1.0
    }
}
