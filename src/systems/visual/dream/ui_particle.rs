use super::components::{DreamGainUiParticle, DreamIconAbsorb, DreamTrailGhost};
use super::dream_bubble_material::DreamBubbleUiMaterial;
use crate::constants::*;
use crate::interface::ui::components::{UiMountSlot, UiNodeRegistry, UiSlot};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;
use rand::Rng;

pub fn ui_particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<DreamBubbleUiMaterial>>,
    ui_nodes: Res<UiNodeRegistry>,
    q_ui_bubble_layer: Query<(Entity, &UiMountSlot)>,
    mut q_icon: Query<&mut DreamIconAbsorb>,
    mut q_particles: Query<(
        Entity,
        &mut DreamGainUiParticle,
        &mut Node,
        &MaterialNode<DreamBubbleUiMaterial>,
        &mut Transform,
    )>,
    q_camera: Query<&Camera, With<crate::interface::camera::MainCamera>>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let ui_bubble_layer = q_ui_bubble_layer
        .iter()
        .find(|(_, slot)| matches!(slot, UiMountSlot::DreamBubbleLayer))
        .map(|(e, _)| e);

    let viewport_size = q_camera
        .iter()
        .next()
        .and_then(|c| c.logical_viewport_size())
        .unwrap_or(Vec2::new(1920., 1080.));

    // 合体ターゲットの位置を事前収集（借用衝突を回避）
    let target_positions: Vec<(Entity, Vec2)> = q_particles
        .iter()
        .map(|(e, _, n, _, _)| {
            let pos = Vec2::new(
                match n.left {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
                match n.top {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
            );
            (e, pos)
        })
        .collect();

    let mut rng = rand::thread_rng();

    for (entity, mut particle, mut node, mat_node, mut transform) in q_particles.iter_mut() {
        particle.time_alive += dt;

        let current_pos = Vec2::new(
            match node.left {
                Val::Px(v) => v,
                _ => 0.0,
            },
            match node.top {
                Val::Px(v) => v,
                _ => 0.0,
            },
        );

        if let Some(target) = particle.merging_into {
            particle.merge_timer -= dt;
            let progress = 1.0 - (particle.merge_timer / DREAM_UI_MERGE_DURATION).clamp(0.0, 1.0);

            if let Some(&(_, target_pos)) = target_positions.iter().find(|(e, _)| *e == target) {
                // Spring-like acceleration instead of lerp
                let to_target = target_pos - current_pos;
                let pull_force = to_target * DREAM_UI_MERGE_PULL_FORCE; // merge attraction
                particle.velocity += pull_force * dt;
                particle.velocity *= DREAM_UI_DRAG;
                let new_pos = current_pos + particle.velocity * dt;

                // Apply boundary damping to merge moves as well
                if new_pos.x < 0.0 {
                    particle.velocity.x *= DREAM_UI_BOUNDARY_DAMPING;
                } else if new_pos.x > viewport_size.x {
                    particle.velocity.x *= DREAM_UI_BOUNDARY_DAMPING;
                }

                if new_pos.y < 0.0 {
                    particle.velocity.y *= DREAM_UI_BOUNDARY_DAMPING;
                } else if new_pos.y > viewport_size.y {
                    particle.velocity.y *= DREAM_UI_BOUNDARY_DAMPING;
                }

                let clamped_pos = new_pos.clamp(Vec2::ZERO, viewport_size);
                node.left = Val::Px(clamped_pos.x);
                node.top = Val::Px(clamped_pos.y);

                let effective_mass = particle.mass + DREAM_UI_BASE_MASS_OFFSET;
                let base = DREAM_UI_PARTICLE_SIZE * effective_mass.sqrt();
                let size = base * (1.0 - progress);
                node.width = Val::Px(size);
                node.height = Val::Px(size);

                if let Some(mat) = materials.get_mut(&mat_node.0) {
                    mat.alpha = 0.9 * (1.0 - progress);
                    mat.time = elapsed;
                    mat.mass = particle.mass;
                }
            }

            if particle.merge_timer <= 0.0 {
                commands.entity(entity).try_despawn();
            }
            continue;
        }

        // 1. Buoyancy (発生直後のみ強く、時間経過で減衰して消える)
        let buoyancy_ratio = (1.0 - (particle.time_alive / 1.5)).max(0.0);
        let buoyancy = Vec2::new(0.0, -DREAM_UI_BUOYANCY * buoyancy_ratio);

        // 2. Attraction
        let to_target = particle.target_pos - current_pos;
        let distance = to_target.length().max(1.0);

        // 距離ベースのみの引力（対数スケールによるなだらかな急加速＋上限クランプ）
        // 大きな泡（質量の大きい泡）ほど強い引力の影響を受ける
        let dist_ratio = (500.0 / distance.max(10.0)).clamp(1.0, 50.0);
        // 1.0 〜 約4.0前後の対数スケールに変換し、最大でも20倍程度の引力までにハードクランプする
        let distance_factor = 1.0 + (dist_ratio.ln() * 3.0).clamp(0.0, 15.0);
        let effective_mass = particle.mass + DREAM_UI_BASE_MASS_OFFSET;
        let attraction_strength = DREAM_UI_BASE_ATTRACTION * distance_factor * effective_mass;
        let mut attraction = to_target.normalize_or_zero() * attraction_strength;

        // 3. Vortex (接線方向の力)
        // 引力に対する渦の比率。合体して大きく・速くなりすぎた泡が円周軌道に入ってしまうのを防ぐため、
        // 近づくほど渦の力を急激に弱め、さらに「大きすぎる泡」ほど直進性を高める。
        let size_dampening = 1.0 / effective_mass.sqrt().max(1.0);
        let vortex_ratio = (distance / 400.0).clamp(0.0, 1.0) * size_dampening;
        // UIの座標系（Yが下方向）において、右上のターゲットへ向かう際に
        // 右下に膨らまないように接線ベクトルの向きを反転 (y, -x)
        let tangent = Vec2::new(to_target.y, -to_target.x).normalize_or_zero();
        attraction += tangent * (attraction_strength * DREAM_UI_VORTEX_STRENGTH * vortex_ratio);

        // 4. Noise
        particle.noise_timer -= dt;
        if particle.noise_timer <= 0.0 {
            let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
            particle.noise_direction = Vec2::new(angle.cos(), angle.sin());
            particle.noise_timer = DREAM_UI_NOISE_INTERVAL;
        }
        // 近づくほどノイズによるブレを減らす
        let noise_ratio = (distance / 400.0).clamp(0.0, 1.0);
        let noise = particle.noise_direction * DREAM_UI_NOISE_STRENGTH * noise_ratio;

        // 5. Boundary Push (弱い斥力)
        // 画面端に近づくにつれて中心に押し戻す弱い力を加える（跳ね返ってしまわない程度）
        let mut boundary = Vec2::ZERO;
        if current_pos.x < DREAM_UI_BOUNDARY_MARGIN {
            let ratio = 1.0 - (current_pos.x.max(0.0) / DREAM_UI_BOUNDARY_MARGIN).clamp(0.0, 1.0);
            boundary.x += DREAM_UI_BOUNDARY_PUSH * ratio;
        } else if current_pos.x > viewport_size.x - DREAM_UI_BOUNDARY_MARGIN {
            let ratio = 1.0 - ((viewport_size.x - current_pos.x).max(0.0) / DREAM_UI_BOUNDARY_MARGIN).clamp(0.0, 1.0);
            boundary.x -= DREAM_UI_BOUNDARY_PUSH * ratio;
        }
        if current_pos.y < DREAM_UI_BOUNDARY_MARGIN {
            let ratio = 1.0 - (current_pos.y.max(0.0) / DREAM_UI_BOUNDARY_MARGIN).clamp(0.0, 1.0);
            boundary.y += DREAM_UI_BOUNDARY_PUSH * ratio;
        } else if current_pos.y > viewport_size.y - DREAM_UI_BOUNDARY_MARGIN {
            // Y軸上端（下端）の斥力も追加
            let ratio = 1.0 - ((viewport_size.y - current_pos.y).max(0.0) / DREAM_UI_BOUNDARY_MARGIN).clamp(0.0, 1.0);
            boundary.y -= DREAM_UI_BOUNDARY_PUSH * ratio;
        }

        // Apply Forces
        let total_force = buoyancy + attraction + noise + boundary;
        particle.velocity += total_force * dt;

        // フレームレート非依存のDrag (60fps基準)
        // アイコンに非常に近い場合は、すり抜けを防ぐために急激なブレーキ（減衰）をかける
        let mut drag = DREAM_UI_DRAG;
        if distance < 50.0 {
            drag = drag.min(DREAM_UI_STRONG_DRAG); // 強いブレーキ
        }
        let drag_factor = drag.powf(dt * 60.0);
        particle.velocity *= drag_factor;

        // 5.5 Minimum velocity (Stuck prevention)
        // 引力と壁の斥力や渦が釣り合って停止・極端な減速をするのを防ぐため、
        // ターゲットへ向かう最低限の速度ベクトルを保証する
        let min_speed = DREAM_UI_MIN_SPEED;
        let speed_toward_target = particle.velocity.dot(to_target.normalize_or_zero());
        if speed_toward_target < min_speed && distance > 20.0 {
            // 足りない分の速度をターゲット方向に対して足す
            let correction = to_target.normalize_or_zero() * (min_speed - speed_toward_target.max(0.0));
            particle.velocity += correction;
        }

        let mut final_pos = current_pos + particle.velocity * dt;

        // Size and Squash & Stretch
        let effective_mass = particle.mass + DREAM_UI_BASE_MASS_OFFSET;
        let base = DREAM_UI_PARTICLE_SIZE * effective_mass.sqrt();
        let speed = particle.velocity.length();
        let squash_stretch_ratio = (speed / DREAM_UI_SQUASH_MAX_SPEED).clamp(0.0, DREAM_UI_SQUASH_MAX_RATIO);
        let length_scale = 1.0 + squash_stretch_ratio;
        let width_scale = 1.0 / (1.0 + squash_stretch_ratio * 0.5);

        // 6. Clamp & Damping (画面外へ出る速度を殺す・跳ね返りはしない)
        if final_pos.x < 0.0 {
            final_pos.x = 0.0;
            particle.velocity.x *= DREAM_UI_BOUNDARY_DAMPING; // 強烈な減衰力
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

        // 6.5 Failsafe Rescue (万が一クランプをすり抜けた場合の強制救済)
        // 浮動小数点演算の誤差や極端な加速で画面外へ大きく吹っ飛んでしまった場合、
        // 画面の反対側（異常値でない側）の境界線上へワープさせる
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

        // 対数スケールによるサイズ縮小（近づくほど急激に小さくなる）
        let start_dist = particle.start_pos.distance(particle.target_pos).max(1.0);

        let dist_ratio = (distance / start_dist.max(100.0)).clamp(0.01, 1.0);
        // log10(dist_ratio * 9.0 + 1.0) で 1.0 -> 1.0, 0.0 -> 0.0 の対数カーブになる
        let shrink = (dist_ratio * 9.0 + 1.0).log10().max(0.1);

        node.width = Val::Px(base * shrink * width_scale);
        node.height = Val::Px(base * shrink * length_scale);

        // Rotation
        if speed > 1.0 {
            let angle = particle.velocity.y.atan2(particle.velocity.x) - std::f32::consts::FRAC_PI_2;
            transform.rotation = Quat::from_rotation_z(angle);
        }

        // Material uniform update
        // 近づく（dist_ratioが小さい）と白っぽく発光する
        let base_color = if dist_ratio < 0.3 {
            let white_t = 1.0 - (dist_ratio / 0.3);
            let r = 0.65 + white_t * 0.35;
            let g = 0.9 + white_t * 0.1;
            LinearRgba::new(r, g, 1.0, 1.0)
        } else {
            LinearRgba::new(0.65, 0.9, 1.0, 1.0)
        };

        // 発生直後はフェードイン
        let alpha = if particle.time_alive < 0.2 {
            (particle.time_alive / 0.2).clamp(0.0, 1.0) * 0.9
        } else {
            0.9
        };

        let vel_dir = if speed > 1.0 {
            particle.velocity / speed
        } else {
            Vec2::ZERO
        };

        if let Some(mat) = materials.get_mut(&mat_node.0) {
            mat.color = base_color;
            mat.alpha = alpha;
            mat.time = elapsed;
            mat.mass = particle.mass;
            mat.velocity_dir = vel_dir;
        }

        // Arrival Check
        if distance < DREAM_UI_ARRIVAL_RADIUS {
            if let Some(icon_entity) = ui_nodes.get_slot(UiSlot::DreamPoolIcon) {
                if let Ok(mut absorb) = q_icon.get_mut(icon_entity) {
                    absorb.pulse_count = absorb.pulse_count.saturating_add(1);
                }
            }
            commands.entity(entity).try_despawn();
            continue;
        }

        // Trail generating
        particle.trail_cooldown -= dt;
        if particle.trail_cooldown <= 0.0 && dist_ratio > 0.15 {
            particle.trail_cooldown = DREAM_UI_TRAIL_INTERVAL;
            let trail_size = base * shrink * DREAM_UI_TRAIL_SIZE_RATIO;
            if let Some(root) = ui_bubble_layer {
                let mut trail_transform = Transform::from_translation(Vec3::ZERO);
                if speed > 1.0 {
                    let angle = particle.velocity.y.atan2(particle.velocity.x) - std::f32::consts::FRAC_PI_2;
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
        }
    }
}

pub fn ui_particle_merge_system(
    mut q_particles: Query<(Entity, &mut DreamGainUiParticle, &Node)>,
) {
    let positions: Vec<(Entity, Vec2, f32, bool)> = q_particles
        .iter()
        .map(|(e, p, n)| {
            let pos = Vec2::new(
                match n.left {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
                match n.top {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
            );
            let t = (p.time_alive / 3.5).clamp(0.0, 1.0);
            let merging = p.merging_into.is_some();
            (e, pos, t, merging)
        })
        .collect();

    let mut merge_pair: Option<(Entity, Entity)> = None;
    'outer: for i in 0..positions.len() {
        if positions[i].3 {
            continue;
        }
        if positions[i].2 < 0.05 {
            continue;
        }
        for j in (i + 1)..positions.len() {
            if positions[j].3 {
                continue;
            }
            if positions[j].2 < 0.05 {
                continue;
            }
            let dist = positions[i].1.distance(positions[j].1);
            if dist < DREAM_UI_MERGE_RADIUS {
                if positions[i].2 < positions[j].2 {
                    merge_pair = Some((positions[i].0, positions[j].0));
                } else {
                    merge_pair = Some((positions[j].0, positions[i].0));
                }
                break 'outer;
            }
        }
    }

    if let Some((absorbed, absorber)) = merge_pair {
        if let Ok([( _, mut absorbed_p, _), ( _, mut absorber_p, _)]) = q_particles.get_many_mut([absorbed, absorber]) {
            // 合体回数だけでなく、質量そのものにも上限を設ける。巨大になりすぎて軌道が壊れるのを防ぐ
            if absorber_p.merge_count >= DREAM_UI_MERGE_MAX_COUNT || absorber_p.mass > DREAM_UI_MERGE_MAX_MASS {
                return;
            }

            absorbed_p.merging_into = Some(absorber);
            absorbed_p.merge_timer = DREAM_UI_MERGE_DURATION;

            absorber_p.merge_count += 1;
            absorber_p.mass += absorbed_p.mass;
        }
    }
}

pub fn dream_trail_ghost_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<DreamBubbleUiMaterial>>,
    mut q_ghosts: Query<(Entity, &mut DreamTrailGhost, &MaterialNode<DreamBubbleUiMaterial>)>,
) {
    let dt = time.delta_secs();
    for (entity, mut ghost, mat_node) in q_ghosts.iter_mut() {
        ghost.lifetime -= dt;
        if ghost.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let alpha = (ghost.lifetime / ghost.max_lifetime) * DREAM_UI_TRAIL_ALPHA;
        if let Some(mat) = materials.get_mut(&mat_node.0) {
            mat.alpha = alpha;
        }
    }
}

pub fn dream_icon_absorb_system(
    time: Res<Time>,
    theme: Res<UiTheme>,
    mut q_icon: Query<(&mut Node, &mut BackgroundColor, &mut DreamIconAbsorb, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut node, mut color, mut absorb, mut transform) in q_icon.iter_mut() {
        if absorb.pulse_count > 0 {
            absorb.timer = DREAM_ICON_ABSORB_DURATION;
            absorb.pulse_count = 0;
        }

        if absorb.timer > 0.0 {
            absorb.timer -= dt;
            let progress = 1.0 - (absorb.timer / DREAM_ICON_ABSORB_DURATION).clamp(0.0, 1.0);
            let sin_val = (progress * std::f32::consts::PI).sin();

            // サイズパルス: 16→20→16
            let size = DREAM_ICON_BASE_SIZE
                + (DREAM_ICON_PULSE_SIZE - DREAM_ICON_BASE_SIZE) * sin_val;
            node.width = Val::Px(size);
            node.height = Val::Px(size);

            // 被弾揺れ（インパクト）
            // 進行方向に逆らうように少し下へ押し込まれる演出
            let impact_offset = (1.0 - progress) * sin_val * 4.0;
            transform.translation.y = impact_offset;

            // 白フラッシュ
            let base = theme.colors.accent_soul_bright;
            let r = base.to_srgba().red + (1.0 - base.to_srgba().red) * sin_val * 0.5;
            let g = base.to_srgba().green + (1.0 - base.to_srgba().green) * sin_val * 0.5;
            let b = base.to_srgba().blue + (1.0 - base.to_srgba().blue) * sin_val * 0.5;
            color.0 = Color::srgb(r, g, b);
        } else {
            node.width = Val::Px(DREAM_ICON_BASE_SIZE);
            node.height = Val::Px(DREAM_ICON_BASE_SIZE);
            transform.translation.y = 0.0;
            color.0 = theme.colors.accent_soul_bright;
        }
    }
}

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
