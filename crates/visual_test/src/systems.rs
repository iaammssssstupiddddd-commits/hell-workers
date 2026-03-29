use bevy::camera_controller::pan_camera::PanCamera;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;
use bevy::window::PrimaryWindow;
use hw_core::constants::{SOUL_GLB_SCALE, VIEW_HEIGHT, Z_OFFSET, Z_RTT_COMPOSITE};
use hw_visual::CharacterMaterial;
use std::time::Duration;

use crate::building::{
    TestBuilding, TestBuilding3dHandles, TestBuilding3dVisual, TestBuildingAssets,
    despawn_test_building_at, spawn_test_building,
};
use crate::soul::{SoulSpawnArgs, spawn_test_soul};
use crate::types::*;

type BtnQuery<'w, 's> = Query<
    'w,
    's,
    (&'static VisualTestAction, &'static Interaction),
    (Changed<Interaction>, With<Button>),
>;

/// VisualTestAction ボタンの押下をハンドリングして TestState を更新する。
#[allow(clippy::too_many_arguments)]
pub fn handle_button_interactions(
    q_btns: BtnQuery,
    mut state: ResMut<TestState>,
    mut elev: ResMut<TestElev>,
    mut commands: Commands,
    assets: Option<Res<TestAssets>>,
    building_assets: Option<Res<TestBuildingAssets>>,
    building_3d_handles: Option<Res<TestBuilding3dHandles>>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    mut q_souls: Query<(
        Entity,
        &mut Transform,
        &TestSoulConfig,
        Option<&SelectedSoul>,
    )>,
    q_buildings: Query<(Entity, &TestBuilding)>,
    q_building_3d: Query<(Entity, &TestBuilding3dVisual)>,
) {
    for (action, interaction) in q_btns.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            VisualTestAction::SetMode(m) => state.mode = m,
            VisualTestAction::NextView => elev.dir = elev.dir.next(),
            VisualTestAction::HeightDown => {
                state.view_height = (state.view_height - 10.0).clamp(50.0, 400.0);
            }
            VisualTestAction::HeightUp => {
                state.view_height = (state.view_height + 10.0).clamp(50.0, 400.0);
            }
            VisualTestAction::OffsetDown => {
                state.z_offset = (state.z_offset - 10.0).clamp(0.0, 400.0);
            }
            VisualTestAction::OffsetUp => {
                state.z_offset = (state.z_offset + 10.0).clamp(0.0, 400.0);
            }
            VisualTestAction::ResetElevation => {
                state.view_height = VIEW_HEIGHT;
                state.z_offset = Z_OFFSET;
            }
            VisualTestAction::SetFace(e) => state.face_mode = FaceMode::Single(e),
            VisualTestAction::SetFaceAll => state.face_mode = FaceMode::AllDifferent,
            VisualTestAction::SetAnimation(i) => state.anim_clip_idx = i,
            VisualTestAction::SetMotion(m) => state.motion = m,
            VisualTestAction::GhostDown => {
                state.ghost_alpha = (state.ghost_alpha - 0.05).clamp(0.0, 1.0);
            }
            VisualTestAction::GhostUp => {
                state.ghost_alpha = (state.ghost_alpha + 0.05).clamp(0.0, 1.0);
            }
            VisualTestAction::RimDown => {
                state.rim_strength = (state.rim_strength - 0.05).clamp(0.0, 2.0);
            }
            VisualTestAction::RimUp => {
                state.rim_strength = (state.rim_strength + 0.05).clamp(0.0, 2.0);
            }
            VisualTestAction::PosterizeDown => {
                state.posterize_steps = (state.posterize_steps - 1.0).clamp(1.0, 16.0);
            }
            VisualTestAction::PosterizeUp => {
                state.posterize_steps = (state.posterize_steps + 1.0).clamp(1.0, 16.0);
            }
            VisualTestAction::ResetShader => {
                state.ghost_alpha = DEFAULT_GHOST_ALPHA;
                state.rim_strength = DEFAULT_RIM_STRENGTH;
                state.posterize_steps = DEFAULT_POSTERIZE_STEPS;
            }
            VisualTestAction::AddSoul => {
                if state.soul_count < MAX_SOULS
                    && let Some(a) = &assets
                {
                    let expr = match state.face_mode {
                        FaceMode::Single(e) => e,
                        FaceMode::AllDifferent => {
                            FaceExpression::ALL[state.soul_count % FaceExpression::ALL.len()]
                        }
                    };
                    spawn_test_soul(
                        &mut commands,
                        &mut character_materials,
                        SoulSpawnArgs {
                            soul_scene: &a.soul_scene,
                            face_atlas: &a.face_atlas,
                            white_pixel: &a.white_pixel,
                            soul_shadow_material: &a.soul_shadow_material,
                            soul_mask_material: &a.soul_mask_material,
                            x: (state.soul_count as f32 - 1.0) * SOUL_SPACING * 0.5,
                            z: 0.0,
                            index: state.next_index,
                            initial_expr: expr,
                            selected: false,
                        },
                    );
                    state.next_index += 1;
                    state.soul_count += 1;
                }
            }
            VisualTestAction::RemoveSoul => {
                if state.soul_count > 1 {
                    let mut cands: Vec<_> = q_souls
                        .iter()
                        .map(|(e, _, cfg, sel)| (e, cfg.index, sel.is_some()))
                        .collect();
                    cands.sort_by_key(|(_, idx, sel)| {
                        (std::cmp::Reverse(*sel as u8), std::cmp::Reverse(*idx))
                    });
                    if let Some(&(entity, _, _)) = cands.first() {
                        commands.entity(entity).despawn();
                        state.soul_count -= 1;
                    }
                }
            }
            VisualTestAction::SelectNextSoul => {
                let mut sorted: Vec<_> = q_souls
                    .iter()
                    .map(|(e, _, cfg, sel)| (e, cfg.index, sel.is_some()))
                    .collect();
                sorted.sort_by_key(|(_, idx, _)| *idx);
                let current = sorted.iter().position(|(_, _, sel)| *sel);
                for &(entity, _, sel) in &sorted {
                    if sel {
                        commands.entity(entity).remove::<SelectedSoul>();
                    }
                }
                let next = current.map(|i| (i + 1) % sorted.len()).unwrap_or(0);
                if let Some(&(entity, _, _)) = sorted.get(next) {
                    commands.entity(entity).insert(SelectedSoul);
                }
            }
            VisualTestAction::ResetSoulPos => {
                let mut sorted: Vec<_> = q_souls.iter_mut().collect();
                sorted.sort_by_key(|(_, _, cfg, _)| cfg.index);
                let n = sorted.len();
                for (i, (_, mut tf, _, _)) in sorted.into_iter().enumerate() {
                    let off = (i as f32) - (n as f32 - 1.0) / 2.0;
                    tf.translation.x = off * SOUL_SPACING;
                    tf.translation.z = 0.0;
                    tf.rotation = Quat::IDENTITY;
                    tf.scale = Vec3::splat(SOUL_GLB_SCALE);
                }
            }
            VisualTestAction::SetBuildingKind(k) => state.building_kind = k,
            VisualTestAction::PlaceOrRemove => {
                let grid = state.building_cursor;
                let occupied = q_buildings.iter().any(|(_, b)| b.grid == grid);
                if occupied {
                    despawn_test_building_at(&mut commands, grid, &q_buildings, &q_building_3d);
                } else if let (Some(ba), Some(bh)) =
                    (building_assets.as_deref(), building_3d_handles.as_deref())
                {
                    spawn_test_building(&mut commands, state.building_kind, grid, ba, bh);
                }
            }
            VisualTestAction::RemoveAllBuildings => {
                for (entity, _) in q_buildings.iter() {
                    commands.entity(entity).despawn();
                }
                for (entity, _) in q_building_3d.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

pub fn apply_faces(
    state: Res<TestState>,
    q_souls: Query<&TestSoulConfig>,
    mut materials: ResMut<Assets<CharacterMaterial>>,
) {
    let mut sorted: Vec<_> = q_souls.iter().collect();
    sorted.sort_by_key(|cfg| cfg.index);
    for (i, config) in sorted.iter().enumerate() {
        let expr = match state.face_mode {
            FaceMode::Single(e) => e,
            FaceMode::AllDifferent => FaceExpression::ALL[i % FaceExpression::ALL.len()],
        };
        if let Some(mat) = materials.get_mut(&config.face_mat) {
            mat.params.uv_offset = expr.uv_offset();
        }
    }
}

pub fn apply_motion(
    state: Res<TestState>,
    mut q_souls: Query<&mut Transform, With<TestSoulConfig>>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs();
    let s = SOUL_GLB_SCALE;
    for mut tf in q_souls.iter_mut() {
        match state.motion {
            MotionMode::Idle => {
                tf.rotation = Quat::IDENTITY;
                tf.scale = Vec3::splat(s);
            }
            MotionMode::FloatingBob => {
                tf.scale = Vec3::new(s, s * (1.0 + (t * 2.0).sin() * 0.05), s);
                tf.rotation = Quat::from_rotation_z((t * 1.5).sin() * 0.08);
            }
            MotionMode::Sleeping => {
                tf.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                tf.scale = Vec3::splat(s * ((t * 0.3).sin() * 0.02 + 1.0));
            }
            MotionMode::Resting => {
                tf.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                tf.scale = Vec3::splat(s * 0.95);
            }
            MotionMode::Escaping => {
                tf.rotation = Quat::from_rotation_z(-0.1);
                tf.scale = Vec3::splat(s * ((t * 8.0).sin() * 0.05 + 0.95));
            }
            MotionMode::Dancing => {
                tf.rotation = Quat::from_rotation_z((t * 5.0).sin() * 0.3);
                tf.scale = Vec3::new(s, s * ((t * 6.0).sin() * 0.15 + 1.0), s);
            }
        }
    }
}

pub fn apply_animation(
    state: Res<TestState>,
    mut q_anim: Query<&mut SoulAnimHandle>,
    mut q_players: AnimPlayerQuery,
) {
    for mut handle in q_anim.iter_mut() {
        if handle.current_playing == state.anim_clip_idx {
            continue;
        }
        let Some(&(_, new_node)) = handle.clips.get(state.anim_clip_idx) else {
            continue;
        };
        let Ok((mut player, mut transitions)) = q_players.get_mut(handle.anim_player_entity) else {
            continue;
        };
        transitions
            .play(&mut player, new_node, Duration::ZERO)
            .repeat();
        handle.current_playing = state.anim_clip_idx;
    }
}

pub fn apply_shader_params(
    state: Res<TestState>,
    q_souls: Query<&TestSoulConfig, With<SelectedSoul>>,
    mut materials: ResMut<Assets<CharacterMaterial>>,
) {
    let Ok(config) = q_souls.single() else { return };
    if let Some(mat) = materials.get_mut(&config.body_mat) {
        mat.params.ghost_alpha = state.ghost_alpha;
        mat.params.rim_strength = state.rim_strength;
        mat.params.posterize_steps = state.posterize_steps;
    }
}

pub fn sync_test_camera3d(
    state: Res<TestState>,
    elev: Res<TestElev>,
    q_cam2d: Cam2dQuery,
    mut q_cam3d: Cam3dSyncQuery,
) {
    let Ok(cam2d) = q_cam2d.single() else { return };
    let scene_z = -cam2d.translation.y;
    let soul_mid_y = SOUL_GLB_SCALE * 0.5;

    let (x_off, y_val, z_off) = match elev.dir {
        TestElevDir::TopDown => (0.0, state.view_height, state.z_offset),
        TestElevDir::North => (0.0, soul_mid_y, ELEV_DISTANCE),
        TestElevDir::South => (0.0, soul_mid_y, -ELEV_DISTANCE),
        TestElevDir::East => (ELEV_DISTANCE, soul_mid_y, 0.0),
        TestElevDir::West => (-ELEV_DISTANCE, soul_mid_y, 0.0),
    };

    for (mut cam3d, mut projection) in &mut q_cam3d {
        cam3d.translation = Vec3::new(cam2d.translation.x + x_off, y_val, scene_z + z_off);
        cam3d.rotation = elev.dir.camera_rotation(state.view_height, state.z_offset);
        cam3d.scale = Vec3::ONE;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = cam2d.scale.x;
        }
    }
}

pub fn apply_composite_sprite(
    state: Res<TestState>,
    elev: Res<TestElev>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_composite: Query<
        (&mut Transform, &MeshMaterial2d<LocalRttCompositeMaterial>),
        With<LocalRttComposite>,
    >,
    mut composite_materials: ResMut<Assets<LocalRttCompositeMaterial>>,
) {
    let Ok((mut tf, mat_handle)) = q_composite.single_mut() else {
        return;
    };
    let Ok(win) = q_window.single() else { return };
    let s = win.size();
    let comp = if elev.dir.is_top_down() {
        state.view_height.hypot(state.z_offset) / state.view_height
    } else {
        1.0
    };
    tf.scale = Vec3::new(s.x, s.y * comp, 1.0);
    tf.translation.z = Z_RTT_COMPOSITE;
    if let Some(mat) = composite_materials.get_mut(&mat_handle.0) {
        mat.params.pixel_size = Vec2::new(1.0 / s.x.max(1.0), 1.0 / s.y.max(1.0));
    }
}

/// パネル上でスクロールしたとき: パネルコンテンツをスクロール & カメラズームを無効化。
/// PreUpdate で InputSystems の後に実行され、PanCamera より先に zoom_speed を制御する。
pub fn handle_panel_scroll(
    q_window: Query<&Window, With<PrimaryWindow>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut q_pan_cam: Query<&mut PanCamera>,
    mut q_scroll: Query<&mut ScrollPosition, With<MenuPanel>>,
    state: Res<TestState>,
) {
    let over_panel = state.menu_visible
        && q_window
            .single()
            .ok()
            .and_then(|w| {
                w.cursor_position()
                    .map(|pos| pos.x > w.width() - MENU_WIDTH)
            })
            .unwrap_or(false);

    for mut cam in &mut q_pan_cam {
        cam.zoom_speed = if over_panel { 0.0 } else { 0.1 };
    }

    if over_panel && scroll.delta != Vec2::ZERO {
        let delta_px = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * 40.0,
            MouseScrollUnit::Pixel => scroll.delta.y,
        };
        if let Ok(mut pos) = q_scroll.single_mut() {
            pos.0.y = (pos.0.y - delta_px).max(0.0);
        }
    }
}
