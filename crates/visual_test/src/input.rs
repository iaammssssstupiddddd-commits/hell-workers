use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::{SOUL_GLB_SCALE, VIEW_HEIGHT, Z_OFFSET};
use hw_visual::CharacterMaterial;

use crate::building::{
    TestBuilding, TestBuilding3dHandles, TestBuilding3dVisual, TestBuildingAssets,
    despawn_test_building_at, spawn_test_building,
};
use crate::soul::{
    SoulRebuildEntities, SoulSpawnArgs, TestSoulShadowCaster, rebuild_soul_test_layout,
    spawn_test_soul,
};
use crate::types::*;

const FACE_KEYS: [KeyCode; 6] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
];

type TestSoulInputQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static TestSoulConfig,
        Option<&'static SelectedSoul>,
    ),
>;

#[derive(SystemParam)]
pub struct SoulInputContext<'w, 's> {
    assets: Option<Res<'w, TestAssets>>,
    character_materials: ResMut<'w, Assets<CharacterMaterial>>,
    layout_entities: SoulLayoutEntities<'w, 's>,
    souls: TestSoulInputQuery<'w, 's>,
    animation_handles: Query<'w, 's, &'static SoulAnimHandle>,
    time: Res<'w, Time>,
}

#[derive(SystemParam)]
pub struct BuildingInputContext<'w, 's> {
    assets: Option<Res<'w, TestBuildingAssets>>,
    handles_3d: Option<Res<'w, TestBuilding3dHandles>>,
    buildings: Query<'w, 's, (Entity, &'static TestBuilding)>,
    visuals_3d: Query<'w, 's, (Entity, &'static TestBuilding3dVisual)>,
}

pub fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<TestState>,
    mut elev: ResMut<TestElev>,
    mut commands: Commands,
    mut soul: SoulInputContext,
    building: BuildingInputContext,
    mut exit: MessageWriter<AppExit>,
) {
    // ── 常時有効 ──────────────────────────────────────────────────────────────
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        state.menu_visible = !state.menu_visible;
    }
    // モード切替
    if keys.just_pressed(KeyCode::Space) {
        state.mode = state.mode.next();
    }

    // カメラ仰角 / 矢視 (常時有効)
    if keys.just_pressed(KeyCode::KeyV) {
        elev.dir = elev.dir.next();
    }
    if keys.just_pressed(KeyCode::KeyJ) {
        state.view_height = (state.view_height - 10.0).clamp(50.0, 400.0);
    }
    if keys.just_pressed(KeyCode::KeyK) {
        state.view_height = (state.view_height + 10.0).clamp(50.0, 400.0);
    }
    if keys.just_pressed(KeyCode::KeyU) {
        state.z_offset = (state.z_offset - 10.0).clamp(0.0, 400.0);
    }
    if keys.just_pressed(KeyCode::KeyI) {
        state.z_offset = (state.z_offset + 10.0).clamp(0.0, 400.0);
    }
    if keys.just_pressed(KeyCode::KeyO) {
        state.view_height = VIEW_HEIGHT;
        state.z_offset = Z_OFFSET;
    }

    // ── モード別 ──────────────────────────────────────────────────────────────
    match state.mode {
        AppMode::Soul => handle_soul_mode(&keys, &mut state, &mut commands, &mut soul),
        AppMode::Build => handle_build_mode(&keys, &mut state, &mut commands, &building),
    }
}

// ─── Soul モード ──────────────────────────────────────────────────────────────

fn handle_soul_mode(
    keys: &ButtonInput<KeyCode>,
    state: &mut TestState,
    commands: &mut Commands,
    soul: &mut SoulInputContext,
) {
    // 表情 [1-6] / [G]
    for (&key, &expr) in FACE_KEYS.iter().zip(FaceExpression::ALL.iter()) {
        if keys.just_pressed(key) {
            state.face_mode = FaceMode::Single(expr);
        }
    }
    if keys.just_pressed(KeyCode::KeyG) {
        state.face_mode = FaceMode::AllDifferent;
    }

    // モーション [M] / アニメーション [Q]
    if keys.just_pressed(KeyCode::KeyM) {
        state.motion = state.motion.next();
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        let n = soul
            .animation_handles
            .iter()
            .next()
            .map(|h| h.clips.len())
            .unwrap_or(ANIM_CLIP_NAMES.len());
        if n > 0 {
            state.anim_clip_idx = (state.anim_clip_idx + 1) % n;
        }
    }

    // シェーダーパラメータ
    if keys.just_pressed(KeyCode::KeyZ) {
        state.ghost_alpha = (state.ghost_alpha - 0.05).clamp(0.0, 1.0);
    }
    if keys.just_pressed(KeyCode::KeyX) {
        state.ghost_alpha = (state.ghost_alpha + 0.05).clamp(0.0, 1.0);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        state.rim_strength = (state.rim_strength - 0.05).clamp(0.0, 2.0);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        state.rim_strength = (state.rim_strength + 0.05).clamp(0.0, 2.0);
    }
    if keys.just_pressed(KeyCode::KeyB) {
        state.posterize_steps = (state.posterize_steps - 1.0).clamp(1.0, 16.0);
    }
    if keys.just_pressed(KeyCode::KeyN) {
        state.posterize_steps = (state.posterize_steps + 1.0).clamp(1.0, 16.0);
    }
    if keys.just_pressed(KeyCode::KeyP) {
        state.ghost_alpha = DEFAULT_GHOST_ALPHA;
        state.rim_strength = DEFAULT_RIM_STRENGTH;
        state.posterize_steps = DEFAULT_POSTERIZE_STEPS;
    }
    if keys.just_pressed(KeyCode::KeyY)
        && let Some(assets) = &soul.assets
    {
        let layout = state.soul_layout.toggle();
        rebuild_soul_test_layout(
            commands,
            &mut soul.character_materials,
            assets.as_ref(),
            state,
            SoulRebuildEntities {
                souls: soul.souls.iter().map(|(entity, _, _, _)| entity).collect(),
                shadows: soul.layout_entities.shadow_proxies.iter().collect(),
                blob_shadows: soul.layout_entities.blob_shadow_proxies.iter().collect(),
                masks: soul.layout_entities.mask_proxies.iter().collect(),
            },
            layout,
        );
        return;
    }

    // Soul 追加 [=]
    if keys.just_pressed(KeyCode::Equal)
        && state.soul_layout == SoulLayout::Default
        && state.soul_count < MAX_SOULS
        && let Some(assets) = &soul.assets
    {
        let initial_expr = match state.face_mode {
            FaceMode::Single(e) => e,
            FaceMode::AllDifferent => {
                FaceExpression::ALL[state.soul_count % FaceExpression::ALL.len()]
            }
        };
        spawn_test_soul(
            commands,
            &mut soul.character_materials,
            SoulSpawnArgs {
                soul_scene: &assets.soul_scene,
                face_atlas: &assets.face_atlas,
                white_pixel: &assets.white_pixel,
                blob_shadow_mesh: &assets.blob_shadow_mesh,
                blob_shadow_material: &assets.blob_shadow_material,
                soul_shadow_material: &assets.soul_shadow_material,
                soul_mask_material: &assets.soul_mask_material,
                shadow_caster: TestSoulShadowCaster::Glb,
                x: (state.soul_count as f32 - 1.0) * SOUL_SPACING * 0.5,
                z: 0.0,
                index: state.next_index,
                initial_expr,
                selected: false,
            },
        );
        state.next_index += 1;
        state.soul_count += 1;
    }

    // Soul 削除 [-]
    if keys.just_pressed(KeyCode::Minus)
        && state.soul_layout == SoulLayout::Default
        && state.soul_count > 1
    {
        let mut candidates: Vec<_> = soul
            .souls
            .iter()
            .map(|(e, _, cfg, sel)| (e, cfg.index, sel.is_some()))
            .collect();
        candidates
            .sort_by_key(|(_, idx, sel)| (std::cmp::Reverse(*sel as u8), std::cmp::Reverse(*idx)));
        if let Some(&(entity, _, _)) = candidates.first() {
            commands.entity(entity).despawn();
            state.soul_count -= 1;
        }
    }

    // 選択切替 [Tab]
    if keys.just_pressed(KeyCode::Tab) {
        let mut sorted: Vec<_> = soul
            .souls
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

    // 位置リセット [R]
    if keys.just_pressed(KeyCode::KeyR) {
        if state.soul_layout == SoulLayout::ShadowCompare
            && let Some(assets) = &soul.assets
        {
            let layout = state.soul_layout;
            rebuild_soul_test_layout(
                commands,
                &mut soul.character_materials,
                assets.as_ref(),
                state,
                SoulRebuildEntities {
                    souls: soul.souls.iter().map(|(entity, _, _, _)| entity).collect(),
                    shadows: soul.layout_entities.shadow_proxies.iter().collect(),
                    blob_shadows: soul.layout_entities.blob_shadow_proxies.iter().collect(),
                    masks: soul.layout_entities.mask_proxies.iter().collect(),
                },
                layout,
            );
            return;
        }
        let mut sorted: Vec<_> = soul.souls.iter_mut().collect();
        sorted.sort_by_key(|(_, _, cfg, _)| cfg.index);
        let n = sorted.len();
        for (i, (_, mut tf, _, _)) in sorted.into_iter().enumerate() {
            let offset = (i as f32) - (n as f32 - 1.0) / 2.0;
            tf.translation.x = offset * SOUL_SPACING;
            tf.translation.z = 0.0;
            tf.rotation = Quat::IDENTITY;
            tf.scale = Vec3::splat(SOUL_GLB_SCALE);
        }
    }

    // 選択ソウル移動 [←→↑↓]
    let speed = 50.0 * soul.time.delta_secs();
    let mut dx = 0.0f32;
    let mut dz = 0.0f32;
    if keys.pressed(KeyCode::ArrowLeft) {
        dx -= speed;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        dx += speed;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        dz -= speed;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        dz += speed;
    }
    if dx != 0.0 || dz != 0.0 {
        for (_, mut tf, _, sel) in soul.souls.iter_mut() {
            if sel.is_some() {
                tf.translation.x += dx;
                tf.translation.z += dz;
            }
        }
    }
}

// ─── Build モード ─────────────────────────────────────────────────────────────

fn handle_build_mode(
    keys: &ButtonInput<KeyCode>,
    state: &mut TestState,
    commands: &mut Commands,
    building: &BuildingInputContext,
) {
    // 建築種別 [[ ] / [ ]]
    if keys.just_pressed(KeyCode::BracketLeft) {
        state.building_kind = state.building_kind.prev();
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        state.building_kind = state.building_kind.next();
    }

    // 配置 / 同位置削除 [Enter]
    if keys.just_pressed(KeyCode::Enter) {
        let grid = state.building_cursor;
        let occupied = building.buildings.iter().any(|(_, b)| b.grid == grid);
        if occupied {
            despawn_test_building_at(commands, grid, &building.buildings, &building.visuals_3d);
        } else if let (Some(ba), Some(bh)) =
            (building.assets.as_deref(), building.handles_3d.as_deref())
        {
            spawn_test_building(commands, state.building_kind, grid, ba, bh);
        }
    }

    // 全削除 [Del]
    if keys.just_pressed(KeyCode::Delete) {
        for (entity, _) in building.buildings.iter() {
            commands.entity(entity).despawn();
        }
        for (entity, _) in building.visuals_3d.iter() {
            commands.entity(entity).despawn();
        }
    }
}
