use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;
use hw_core::camera::MainCamera;
use hw_core::constants::*;
use hw_core::ui_nodes::{UiMountSlot, UiNodeRegistry, UiSlot};
use rand::Rng;

use super::super::components::{DreamGainUiParticle, DreamIconAbsorb};
use super::super::dream_bubble_material::DreamBubbleUiMaterial;

#[derive(SystemParam)]
pub struct UiParticleReadParams<'w, 's> {
    q_ui_bubble_layer: Query<'w, 's, (Entity, &'static UiMountSlot)>,
    q_camera: Query<'w, 's, &'static Camera, With<MainCamera>>,
    ui_nodes: Res<'w, UiNodeRegistry>,
}

type ParticlesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut DreamGainUiParticle,
        &'static mut Node,
        &'static MaterialNode<DreamBubbleUiMaterial>,
        &'static mut Transform,
    ),
>;

pub fn ui_particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<DreamBubbleUiMaterial>>,
    read: UiParticleReadParams,
    mut q_icon: Query<&mut DreamIconAbsorb>,
    mut q_particles: ParticlesQuery,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let ui_bubble_layer = read
        .q_ui_bubble_layer
        .iter()
        .find(|(_, slot)| matches!(slot, UiMountSlot::DreamBubbleLayer))
        .map(|(e, _)| e);

    let viewport_size = read
        .q_camera
        .iter()
        .next()
        .and_then(|c| c.logical_viewport_size())
        .unwrap_or(Vec2::new(1920., 1080.));

    let target_positions: Vec<(Entity, Vec2)> = q_particles
        .iter()
        .map(|(e, _, n, _, _)| (e, ui_position_from_node(n)))
        .collect();

    let mut rng = rand::thread_rng();

    let icon_entity = read.ui_nodes.get_slot(UiSlot::DreamPoolIcon);

    for (entity, mut particle, mut node, mat_node, mut transform) in q_particles.iter_mut() {
        particle.time_alive += dt;

        let current_pos = ui_position_from_node(&node);

        let should_despawn = if particle.merging_into.is_some() {
            update_merging_particle(
                MergeInput { dt, elapsed, viewport_size, current_pos },
                &target_positions,
                &mut particle,
                &mut node,
                mat_node,
                &mut materials,
            )
        } else {
            let arrived = update_standard::update_standard_particle(
                update_standard::StandardInput { dt, elapsed, viewport_size, current_pos },
                update_standard::ParticleState { particle: &mut particle, rng: &mut rng },
                update_standard::NodeVisuals { node: &mut node, mat_node, transform: &mut transform },
                &mut materials,
                ui_bubble_layer,
                &mut commands,
            );
            if arrived
                && let Some(icon_e) = icon_entity
                    && let Ok(mut absorb) = q_icon.get_mut(icon_e)
                {
                    absorb.pulse_count = absorb.pulse_count.saturating_add(1);
                }
            arrived
        };

        if should_despawn {
            commands.entity(entity).try_despawn();
        }
    }
}

fn ui_position_from_node(node: &Node) -> Vec2 {
    Vec2::new(
        match node.left {
            Val::Px(v) => v,
            _ => 0.0,
        },
        match node.top {
            Val::Px(v) => v,
            _ => 0.0,
        },
    )
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

struct MergeInput {
    dt: f32,
    elapsed: f32,
    viewport_size: Vec2,
    current_pos: Vec2,
}

fn update_merging_particle(
    input: MergeInput,
    target_positions: &[(Entity, Vec2)],
    particle: &mut DreamGainUiParticle,
    node: &mut Node,
    mat_node: &MaterialNode<DreamBubbleUiMaterial>,
    materials: &mut Assets<DreamBubbleUiMaterial>,
) -> bool {
    let MergeInput { dt, elapsed, viewport_size, current_pos } = input;
    particle.merge_timer -= dt;
    let progress = 1.0 - (particle.merge_timer / DREAM_UI_MERGE_DURATION).clamp(0.0, 1.0);

    if let Some(target_entity) = particle.merging_into
        && let Some((_, target_pos)) = target_positions.iter().find(|(e, _)| *e == target_entity) {
            let to_target = *target_pos - current_pos;
            let pull_force = to_target * DREAM_UI_MERGE_PULL_FORCE;
            particle.velocity += pull_force * dt;
            particle.velocity *= DREAM_UI_DRAG;
            let new_pos = current_pos + particle.velocity * dt;

            if new_pos.x < 0.0 || new_pos.x > viewport_size.x {
                particle.velocity.x *= DREAM_UI_BOUNDARY_DAMPING;
            }

            if new_pos.y < 0.0 || new_pos.y > viewport_size.y {
                particle.velocity.y *= DREAM_UI_BOUNDARY_DAMPING;
            }

            let clamped_pos = new_pos.clamp(Vec2::ZERO, viewport_size);
            node.left = Val::Px(clamped_pos.x);
            node.top = Val::Px(clamped_pos.y);

            let cluster_scale = merge_cluster_scale(particle.mass);

            let effective_mass = particle.mass + DREAM_UI_BASE_MASS_OFFSET;
            let base = DREAM_UI_PARTICLE_SIZE * effective_mass.sqrt() * cluster_scale;
            let size = base * (1.0 - progress);
            node.width = Val::Px(size);
            node.height = Val::Px(size);

            if let Some(mat) = materials.get_mut(&mat_node.0) {
                mat.alpha = 0.9 * (1.0 - progress);
                mat.time = elapsed;
                mat.mass = particle.mass;
            }
            return particle.merge_timer <= 0.0;
        }

    if particle.merge_timer <= 0.0 {
        return true;
    }

    node.left = Val::Px(current_pos.x);
    node.top = Val::Px(current_pos.y);
    false
}

mod update_standard;
mod update_trail;

pub fn spawn_ui_particle(
    commands: &mut Commands,
    start_pos: Vec2,
    target_pos: Vec2,
    ui_root: Entity,
    materials: &mut Assets<DreamBubbleUiMaterial>,
    mass: f32,
) {
    let mut rng = rand::thread_rng();

    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let noise_dir = Vec2::new(angle.cos(), angle.sin());

    let particle = commands
        .spawn((
            DreamGainUiParticle {
                time_alive: 0.0,
                start_pos,
                target_pos,
                velocity: Vec2::new(rng.gen_range(-40.0..40.0), rng.gen_range(-60.0..-15.0)),
                phase: rng.gen_range(0.0..std::f32::consts::TAU),
                noise_direction: noise_dir,
                noise_timer: rng.gen_range(0.0..DREAM_UI_NOISE_INTERVAL),
                merge_count: 0,
                merging_into: None,
                merge_timer: 0.0,
                trail_cooldown: DREAM_UI_TRAIL_INTERVAL,
                prev_pos: start_pos,
                mass,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(start_pos.x),
                top: Val::Px(start_pos.y),
                width: Val::Px(DREAM_UI_PARTICLE_SIZE),
                height: Val::Px(DREAM_UI_PARTICLE_SIZE),
                ..default()
            },
            Transform::default(),
            MaterialNode(materials.add(DreamBubbleUiMaterial {
                color: LinearRgba::new(0.65, 0.9, 1.0, 1.0),
                time: 0.0,
                alpha: 0.0,
                mass,
                velocity_dir: Vec2::ZERO,
            })),
            GlobalZIndex(-1),
            Name::new("DreamGainUiParticle"),
        ))
        .id();

    commands.entity(ui_root).add_child(particle);
}
