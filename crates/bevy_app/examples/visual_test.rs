//! Soul GLB Visual Test Scene
//!
//! ゲーム本体とは独立して、表情アトラス・モーション・Z-fight を検証する。
//!
//! 操作:
//!   [1]-[6]   表情切り替え (Normal/Fear/Exhausted/Concentration/Happy/Sleep)
//!   [A]       全表情モード (各 Soul に異なる表情を自動割当)
//!   [+/=]     Soul 追加 (最大 6)
//!   [-]       Soul 削除
//!   [M]       モーション切り替え
//!   [R]       全ポジションリセット
//!   [←→↑↓]   選択 Soul の移動
//!   [Tab]     Soul 選択切り替え
//!   [Esc]     終了
//!
//! ```bash
//! CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo run --example visual_test -p bevy_app
//! ```

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::gltf::GltfMeshName;
use bevy::mesh::Mesh3d;
use bevy::pbr::{MaterialPlugin, MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::scene::SceneInstanceReady;
use bevy::window::PrimaryWindow;

use hw_core::constants::{
    LAYER_3D, LAYER_OVERLAY, SOUL_FACE_SCALE_MULTIPLIER, SOUL_GLB_SCALE, VIEW_HEIGHT, Z_OFFSET,
};
use hw_visual::CharacterMaterial;

// ─── Constants ─────────────────────────────────────────────────────────────

const SOUL_SPACING: f32 = SOUL_GLB_SCALE * 2.5;

// ─── Face atlas constants (mirrors visual_handles.rs) ──────────────────────

const ATLAS_COLS: f32 = 3.0;
const ATLAS_ROWS: f32 = 2.0;
const CELL_PX: f32 = 256.0;
const CROP_PX: f32 = 152.0;
const MAG: f32 = 1.4;
const CROP_OX: f32 = 24.0;
const CROP_OY: f32 = 32.0;

fn face_uv_scale() -> Vec2 {
    Vec2::new(
        CROP_PX / MAG / CELL_PX / ATLAS_COLS,
        CROP_PX / MAG / CELL_PX / ATLAS_ROWS,
    )
}

fn face_uv_offset(col: f32, row: f32) -> Vec2 {
    let center_adjust = (CROP_PX - CROP_PX / MAG) * 0.5;
    Vec2::new(
        (col * CELL_PX + CROP_OX + center_adjust) / (CELL_PX * ATLAS_COLS),
        (row * CELL_PX + CROP_OY + center_adjust) / (CELL_PX * ATLAS_ROWS),
    )
}

// ─── Enums ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FaceExpression {
    #[default]
    Normal,
    Fear,
    Exhausted,
    Concentration,
    Happy,
    Sleep,
}

impl FaceExpression {
    const ALL: [Self; 6] = [
        Self::Normal,
        Self::Fear,
        Self::Exhausted,
        Self::Concentration,
        Self::Happy,
        Self::Sleep,
    ];

    fn uv_offset(self) -> Vec2 {
        let (col, row) = match self {
            Self::Normal => (0.0, 0.0),
            Self::Fear => (1.0, 0.0),
            Self::Exhausted => (2.0, 0.0),
            Self::Concentration => (0.0, 1.0),
            Self::Happy => (1.0, 1.0),
            Self::Sleep => (2.0, 1.0),
        };
        face_uv_offset(col, row)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Fear => "Fear",
            Self::Exhausted => "Exhausted",
            Self::Concentration => "Concentration",
            Self::Happy => "Happy",
            Self::Sleep => "Sleep",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum MotionMode {
    #[default]
    Idle,
    FloatingBob,
    Sleeping,
    Resting,
    Escaping,
    Dancing,
}

impl MotionMode {
    fn next(self) -> Self {
        match self {
            Self::Idle => Self::FloatingBob,
            Self::FloatingBob => Self::Sleeping,
            Self::Sleeping => Self::Resting,
            Self::Resting => Self::Escaping,
            Self::Escaping => Self::Dancing,
            Self::Dancing => Self::Idle,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::FloatingBob => "FloatingBob",
            Self::Sleeping => "Sleeping",
            Self::Resting => "Resting",
            Self::Escaping => "Escaping",
            Self::Dancing => "Dancing",
        }
    }
}

// ─── Components / Resources ────────────────────────────────────────────────

#[derive(Component)]
struct Camera3dRtt;

#[derive(Component)]
struct TestSoulConfig {
    face_mat: Handle<CharacterMaterial>,
    body_mat: Handle<CharacterMaterial>,
    index: usize,
}

#[derive(Component)]
struct SelectedSoul;

#[derive(Component)]
struct HudText;

#[derive(Resource)]
struct TestAssets {
    soul_scene: Handle<Scene>,
    face_atlas: Handle<Image>,
    white_pixel: Handle<Image>,
}

#[derive(Resource)]
struct TestState {
    face_mode: FaceMode,
    motion: MotionMode,
    soul_count: usize,
    next_index: usize,
}

#[derive(Clone, Copy, Debug)]
enum FaceMode {
    Single(FaceExpression),
    AllDifferent,
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            face_mode: FaceMode::Single(FaceExpression::Normal),
            motion: MotionMode::Idle,
            soul_count: 0,
            next_index: 0,
        }
    }
}

// ─── Plugin ────────────────────────────────────────────────────────────────

struct VisualTestPlugin;

impl Plugin for VisualTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TestState>()
            .add_systems(Startup, setup_scene)
            .add_observer(on_soul_scene_ready)
            .add_systems(
                Update,
                (keyboard_input, apply_faces, apply_motion, update_hud).chain(),
            );
    }
}

// ─── Startup ───────────────────────────────────────────────────────────────

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<TestState>,
) {
    // --- RtT texture ---
    let (w, h) = q_window
        .single()
        .map(|win| (win.physical_width().max(1), win.physical_height().max(1)))
        .unwrap_or((1280, 720));
    let rtt_image = Image::new_target_texture(
        w,
        h,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    let rtt_handle = images.add(rtt_image);

    // --- Camera3d (RtT offscreen) ---
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        AmbientLight {
            brightness: 500.0,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_3d()),
        {
            let t = Transform::from_xyz(0.0, VIEW_HEIGHT, Z_OFFSET);
            t.looking_at(Vec3::ZERO, Vec3::NEG_Z)
        },
        RenderTarget::Image(rtt_handle.clone().into()),
        RenderLayers::layer(LAYER_3D),
        Camera3dRtt,
    ));

    // --- Camera2d (main screen output) ---
    commands.spawn((Camera2d, RenderLayers::layer(LAYER_OVERLAY)));

    // --- Composite sprite ---
    commands.spawn((
        Sprite {
            image: rtt_handle,
            custom_size: q_window.single().ok().map(|win| {
                let s = win.size();
                let comp = VIEW_HEIGHT.hypot(Z_OFFSET) / VIEW_HEIGHT;
                Vec2::new(s.x, s.y * comp)
            }),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        RenderLayers::layer(LAYER_OVERLAY),
    ));

    // --- Assets ---
    let soul_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/characters/soul.glb"));
    let face_atlas = asset_server.load("textures/character/soul_face_atlas.png");
    let white_pixel = images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ));
    let font: Handle<Font> = asset_server.load("fonts/NotoSansJP-VF.ttf");

    commands.insert_resource(TestAssets {
        soul_scene: soul_scene.clone(),
        face_atlas: face_atlas.clone(),
        white_pixel: white_pixel.clone(),
    });

    // --- Spawn initial 3 souls ---
    for i in 0..3 {
        spawn_test_soul(
            &mut commands,
            &mut character_materials,
            SoulSpawnArgs {
                soul_scene: &soul_scene,
                face_atlas: &face_atlas,
                white_pixel: &white_pixel,
                x: (i as f32 - 1.0) * SOUL_SPACING,
                z: 0.0,
                index: state.next_index,
                initial_expr: FaceExpression::Normal,
                selected: i == 0,
            },
        );
        state.next_index += 1;
        state.soul_count += 1;
    }

    // --- HUD ---
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font,
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                HudText,
            ));
        });
}

// ─── Soul spawning ─────────────────────────────────────────────────────────

const MAX_SOULS: usize = 6;

struct SoulSpawnArgs<'a> {
    soul_scene: &'a Handle<Scene>,
    face_atlas: &'a Handle<Image>,
    white_pixel: &'a Handle<Image>,
    x: f32,
    z: f32,
    index: usize,
    initial_expr: FaceExpression,
    selected: bool,
}

fn spawn_test_soul(
    commands: &mut Commands,
    character_materials: &mut Assets<CharacterMaterial>,
    args: SoulSpawnArgs,
) {
    let face_mat = character_materials.add(CharacterMaterial::face(
        args.face_atlas.clone(),
        LinearRgba::WHITE,
        face_uv_scale(),
        args.initial_expr.uv_offset(),
    ));
    let body_mat = character_materials.add(CharacterMaterial::body(args.white_pixel.clone()));

    let mut entity = commands.spawn((
        SceneRoot(args.soul_scene.clone()),
        Transform::from_xyz(args.x, 0.0, args.z).with_scale(Vec3::splat(SOUL_GLB_SCALE)),
        RenderLayers::layer(LAYER_3D),
        TestSoulConfig {
            face_mat,
            body_mat,
            index: args.index,
        },
    ));
    if args.selected {
        entity.insert(SelectedSoul);
    }
}

// ─── GLB material replacement observer ─────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn on_soul_scene_ready(
    scene_ready: On<SceneInstanceReady>,
    q_configs: Query<&TestSoulConfig>,
    q_children: Query<&Children>,
    q_mesh_names: Query<&GltfMeshName>,
    q_names: Query<&Name>,
    q_meshes: Query<(), With<Mesh3d>>,
    q_transforms: Query<&Transform>,
    mut commands: Commands,
) {
    let Ok(config) = q_configs.get(scene_ready.entity) else {
        return;
    };

    let render_layers = RenderLayers::layer(LAYER_3D);
    for child in q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert(render_layers.clone());

        if q_meshes.get(child).is_err() {
            continue;
        }

        let mesh_name = q_mesh_names.get(child).ok().map(|name| name.0.as_str());
        let name = q_names.get(child).ok().map(Name::as_str);

        let is_face_mesh =
            matches!(mesh_name, Some("Soul_Face_Mesh")) || matches!(name, Some("Soul_Face_Mesh"));
        if is_face_mesh {
            if let Ok(face_transform) = q_transforms.get(child) {
                let mut scaled = *face_transform;
                scaled.scale *=
                    Vec3::new(SOUL_FACE_SCALE_MULTIPLIER, SOUL_FACE_SCALE_MULTIPLIER, 1.0);
                entity_commands.insert(scaled);
            }
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(config.face_mat.clone()));
            continue;
        }

        let is_body_mesh = matches!(mesh_name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010.SoulMat"));
        if is_body_mesh {
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d::<CharacterMaterial>(config.body_mat.clone()));
        }
    }
}

// ─── Input handling ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<TestState>,
    mut commands: Commands,
    assets: Option<Res<TestAssets>>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    mut q_souls: Query<(
        Entity,
        &mut Transform,
        &TestSoulConfig,
        Option<&SelectedSoul>,
    )>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time>,
) {
    // Esc → quit
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    // Face expression keys [1]-[6]
    for (i, expr) in FaceExpression::ALL.iter().enumerate() {
        let key = match i {
            0 => KeyCode::Digit1,
            1 => KeyCode::Digit2,
            2 => KeyCode::Digit3,
            3 => KeyCode::Digit4,
            4 => KeyCode::Digit5,
            5 => KeyCode::Digit6,
            _ => unreachable!(),
        };
        if keys.just_pressed(key) {
            state.face_mode = FaceMode::Single(*expr);
        }
    }

    // [A] → all-different mode
    if keys.just_pressed(KeyCode::KeyA) {
        state.face_mode = FaceMode::AllDifferent;
    }

    // [M] → cycle motion
    if keys.just_pressed(KeyCode::KeyM) {
        state.motion = state.motion.next();
    }

    // [R] → reset positions
    if keys.just_pressed(KeyCode::KeyR) {
        let mut sorted: Vec<_> = q_souls.iter_mut().collect();
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

    // [+/=] → add soul
    if keys.just_pressed(KeyCode::Equal)
        && state.soul_count < MAX_SOULS
        && let Some(ref assets) = assets
    {
        let initial_expr = match state.face_mode {
            FaceMode::Single(e) => e,
            FaceMode::AllDifferent => {
                FaceExpression::ALL[state.soul_count % FaceExpression::ALL.len()]
            }
        };
        spawn_test_soul(
            &mut commands,
            &mut character_materials,
            SoulSpawnArgs {
                soul_scene: &assets.soul_scene,
                face_atlas: &assets.face_atlas,
                white_pixel: &assets.white_pixel,
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

    // [-] → remove last soul (non-selected preferred)
    if keys.just_pressed(KeyCode::Minus) && state.soul_count > 1 {
        let mut candidates: Vec<_> = q_souls
            .iter()
            .map(|(e, _, cfg, sel)| (e, cfg.index, sel.is_some()))
            .collect();
        candidates.sort_by_key(|(_, idx, selected)| {
            (std::cmp::Reverse(*selected as u8), std::cmp::Reverse(*idx))
        });
        // Pick the first non-selected with highest index, or the highest index if all selected
        if let Some(&(entity, _, _)) = candidates.first() {
            commands.entity(entity).despawn();
            state.soul_count -= 1;
        }
    }

    // [Tab] → cycle selection
    if keys.just_pressed(KeyCode::Tab) {
        let mut sorted: Vec<_> = q_souls
            .iter()
            .map(|(e, _, cfg, sel)| (e, cfg.index, sel.is_some()))
            .collect();
        sorted.sort_by_key(|(_, idx, _)| *idx);

        let current = sorted.iter().position(|(_, _, sel)| *sel);
        // Remove current selection
        for &(entity, _, sel) in &sorted {
            if sel {
                commands.entity(entity).remove::<SelectedSoul>();
            }
        }
        // Select next
        let next_idx = current.map(|i| (i + 1) % sorted.len()).unwrap_or(0);
        if let Some(&(entity, _, _)) = sorted.get(next_idx) {
            commands.entity(entity).insert(SelectedSoul);
        }
    }

    // Arrow keys → move selected soul
    let speed = 50.0 * time.delta_secs();
    for (_, mut tf, _, sel) in q_souls.iter_mut() {
        if sel.is_none() {
            continue;
        }
        if keys.pressed(KeyCode::ArrowLeft) {
            tf.translation.x -= speed;
        }
        if keys.pressed(KeyCode::ArrowRight) {
            tf.translation.x += speed;
        }
        if keys.pressed(KeyCode::ArrowUp) {
            tf.translation.z -= speed;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            tf.translation.z += speed;
        }
    }
}

// ─── Face application ──────────────────────────────────────────────────────

fn apply_faces(
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

// ─── Motion application ────────────────────────────────────────────────────

fn apply_motion(
    state: Res<TestState>,
    mut q_souls: Query<&mut Transform, With<TestSoulConfig>>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs();
    let base_scale = SOUL_GLB_SCALE;

    for mut tf in q_souls.iter_mut() {
        match state.motion {
            MotionMode::Idle => {
                tf.rotation = Quat::IDENTITY;
                tf.scale = Vec3::splat(base_scale);
            }
            MotionMode::FloatingBob => {
                let bob = (t * 2.0).sin() * 0.05;
                tf.scale = Vec3::new(base_scale, base_scale * (1.0 + bob), base_scale);
                tf.rotation = Quat::from_rotation_z((t * 1.5).sin() * 0.08);
            }
            MotionMode::Sleeping => {
                tf.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                let breath = (t * 0.3).sin() * 0.02 + 1.0;
                tf.scale = Vec3::splat(base_scale * breath);
            }
            MotionMode::Resting => {
                tf.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                tf.scale = Vec3::splat(base_scale * 0.95);
            }
            MotionMode::Escaping => {
                tf.rotation = Quat::from_rotation_z(-0.1);
                let panic = (t * 8.0).sin() * 0.05 + 0.95;
                tf.scale = Vec3::splat(base_scale * panic);
            }
            MotionMode::Dancing => {
                let sway = (t * 5.0).sin() * 0.3;
                tf.rotation = Quat::from_rotation_z(sway);
                let bounce = (t * 6.0).sin() * 0.15 + 1.0;
                tf.scale = Vec3::new(base_scale, base_scale * bounce, base_scale);
            }
        }
    }
}

// ─── HUD update ────────────────────────────────────────────────────────────

fn update_hud(
    state: Res<TestState>,
    q_selected: Query<&TestSoulConfig, With<SelectedSoul>>,
    mut q_hud: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = q_hud.single_mut() else {
        return;
    };

    let face_label = match state.face_mode {
        FaceMode::Single(e) => format!("Face: {}", e.label()),
        FaceMode::AllDifferent => "Face: AllDifferent".to_string(),
    };
    let sel_label = q_selected
        .single()
        .ok()
        .map(|c| format!("Soul #{}", c.index))
        .unwrap_or_else(|| "None".to_string());

    **text = format!(
        "{} | Motion: {} | Selected: {} | Souls: {}/{}",
        face_label,
        state.motion.label(),
        sel_label,
        state.soul_count,
        MAX_SOULS,
    );
}

// ─── main ──────────────────────────────────────────────────────────────────

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Visual Test — Soul GLB".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MaterialPlugin::<CharacterMaterial>::default())
        .add_plugins(VisualTestPlugin)
        .run();
}
