//! Soul GLB Visual Test Scene
//!
//! ゲーム本体とは独立して、表情アトラス・モーション・Z-fight を検証する。
//! 右側のメニューパネルに操作一覧と現在値を常時表示。[H] でパネルを折りたたみ。
//!
//! ```bash
//! CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo run --example visual_test -p bevy_app
//! ```

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::camera_controller::pan_camera::{PanCamera, PanCameraPlugin};
use bevy::gltf::{Gltf, GltfMeshName};
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster, NotShadowReceiver};
use bevy::mesh::Mesh3d;
use bevy::pbr::{MaterialPlugin, MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::scene::SceneInstanceReady;
use bevy::window::PrimaryWindow;
use std::fmt::Write as _;
use std::time::Duration;

use hw_core::constants::{
    LAYER_2D, LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW, LAYER_OVERLAY, MAP_HEIGHT,
    MAP_WIDTH, SOUL_FACE_SCALE_MULTIPLIER, SOUL_GLB_SCALE,
    SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES, TILE_SIZE, VIEW_HEIGHT, Z_MAP, Z_MAP_DIRT,
    Z_MAP_GRASS, Z_MAP_SAND, Z_OFFSET, topdown_sun_direction_world,
};
use hw_visual::{CharacterMaterial, SoulShadowMaterial};
use hw_visual::visual3d::SoulShadowProxy3d;
use hw_world::{TerrainType, generate_base_terrain_tiles, grid_to_world, SAND_WIDTH};

// ─── Constants ─────────────────────────────────────────────────────────────

const SOUL_SPACING: f32 = SOUL_GLB_SCALE * 2.5;
const MENU_WIDTH: f32 = 270.0;

/// GLB に含まれるアニメーションクリップ名（[Q] で順番に切り替え）
const ANIM_CLIP_NAMES: &[&str] = &[
    "Idle",
    "Walk",
    "Work",
    "Carry",
    "Fear",
    "Exhausted",
    "WalkLeft",
    "WalkRight",
];

/// `CharacterMaterial::body` のデフォルト値（[P] リセット用）
const DEFAULT_GHOST_ALPHA: f32 = 1.0;
const DEFAULT_RIM_STRENGTH: f32 = 0.28;
const DEFAULT_POSTERIZE_STEPS: f32 = 4.0;

/// 矢視（Elevation）モード時の Camera3d のシーン中心からの距離
const ELEV_DISTANCE: f32 = 200.0;

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
    const ALL: [Self; 6] = [
        Self::Idle,
        Self::FloatingBob,
        Self::Sleeping,
        Self::Resting,
        Self::Escaping,
        Self::Dancing,
    ];

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

/// 矢視方向（ゲーム本体の ElevationDirection 相当）
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TestElevDir {
    #[default]
    TopDown,
    North,
    East,
    South,
    West,
}

impl TestElevDir {
    fn next(self) -> Self {
        match self {
            Self::TopDown => Self::North,
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::TopDown,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::TopDown => "TopDown",
            Self::North => "North",
            Self::East => "East",
            Self::South => "South",
            Self::West => "West",
        }
    }

    fn is_top_down(self) -> bool {
        self == Self::TopDown
    }

    fn camera_rotation(self, view_height: f32, z_offset: f32) -> Quat {
        match self {
            Self::TopDown => {
                Transform::from_xyz(0.0, view_height, z_offset)
                    .looking_at(Vec3::ZERO, Vec3::NEG_Z)
                    .rotation
            }
            Self::North => {
                Transform::from_xyz(0.0, 0.0, 1.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
            Self::South => {
                Transform::from_xyz(0.0, 0.0, -1.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
            Self::East => {
                Transform::from_xyz(1.0, 0.0, 0.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
            Self::West => {
                Transform::from_xyz(-1.0, 0.0, 0.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
        }
    }
}

/// 矢視状態リソース（ゲーム本体の ElevationViewState 相当）
#[derive(Resource, Default)]
struct TestElev {
    dir: TestElevDir,
}

// ─── Components / Resources ────────────────────────────────────────────────

#[derive(Component)]
struct Camera3dRtt;

/// PanCamera（WASD パン・スクロールズーム）を持つ Camera2d のマーカー
#[derive(Component)]
struct TestMainCamera;

#[derive(Component)]
struct CompositeSprite;

#[derive(Component)]
struct TestSoulConfig {
    face_mat: Handle<CharacterMaterial>,
    body_mat: Handle<CharacterMaterial>,
    index: usize,
}

#[derive(Component)]
struct SelectedSoul;

/// 右サイドメニューパネルのルートノード
#[derive(Component)]
struct MenuPanel;

/// メニューパネル内のテキスト
#[derive(Component)]
struct MenuText;

/// パネル非表示時に右上に表示する「[H] メニュー表示」ヒント
#[derive(Component)]
struct MenuHint;

/// ワールドマップタイルのマーカーコンポーネント。
#[derive(Component)]
struct WorldMapTile;

/// ソウルのシャドウプロキシが使用するシャドウマテリアルを保持するコンポーネント。
#[derive(Component)]
struct SoulShadowConfig {
    shadow_mat: Handle<SoulShadowMaterial>,
}

/// アニメーション再生に必要な情報を Soul ルートエンティティに保持するコンポーネント。
#[derive(Component)]
struct SoulAnimHandle {
    anim_player_entity: Entity,
    clips: Vec<(&'static str, AnimationNodeIndex)>,
    current_playing: usize,
}

#[derive(Resource)]
struct TestAssets {
    soul_scene: Handle<Scene>,
    face_atlas: Handle<Image>,
    white_pixel: Handle<Image>,
    gltf_handle: Handle<Gltf>,
    soul_shadow_material: Handle<SoulShadowMaterial>,
}

#[derive(Resource)]
struct TestState {
    face_mode: FaceMode,
    motion: MotionMode,
    soul_count: usize,
    next_index: usize,
    anim_clip_idx: usize,
    view_height: f32,
    z_offset: f32,
    ghost_alpha: f32,
    rim_strength: f32,
    posterize_steps: f32,
    menu_visible: bool,
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
            anim_clip_idx: 0,
            view_height: VIEW_HEIGHT,
            z_offset: Z_OFFSET,
            ghost_alpha: DEFAULT_GHOST_ALPHA,
            rim_strength: DEFAULT_RIM_STRENGTH,
            posterize_steps: DEFAULT_POSTERIZE_STEPS,
            menu_visible: true,
        }
    }
}

// ─── Query type aliases ────────────────────────────────────────────────────

type AnimPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut AnimationPlayer,
        &'static mut AnimationTransitions,
    ),
>;

type Cam3dSyncQuery<'w, 's> =
    Query<'w, 's, (&'static mut Transform, &'static mut Projection), With<Camera3dRtt>>;

// ─── Plugin ────────────────────────────────────────────────────────────────

struct VisualTestPlugin;

impl Plugin for VisualTestPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PanCameraPlugin,
            MaterialPlugin::<SoulShadowMaterial>::default(),
        ))
        .init_resource::<TestState>()
        .init_resource::<TestElev>()
        .add_systems(Startup, (setup_scene, setup_world_map))
        .add_observer(on_soul_scene_ready)
        .add_observer(on_shadow_scene_ready)
        .add_systems(
            Update,
            (
                keyboard_input,
                sync_test_camera3d,
                apply_faces,
                apply_motion,
                apply_animation,
                apply_shader_params,
                apply_composite_sprite,
                apply_menu_visibility,
                update_hud,
            )
                .chain(),
        );
    }
}

// ─── Startup ───────────────────────────────────────────────────────────────

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    mut soul_shadow_materials: ResMut<Assets<SoulShadowMaterial>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<TestState>,
) {
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });

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

    // --- Camera2d (TestMainCamera: W/A/S/D パン + スクロールズーム) ---
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        RenderLayers::layer(LAYER_2D),
        TestMainCamera,
        PanCamera {
            key_rotate_ccw: None, // [Q] はアニメーション切替に使用
            key_rotate_cw: None,
            key_zoom_in: None,  // [=] は Soul 追加に使用
            key_zoom_out: None, // [-] は Soul 削除に使用
            ..Default::default()
        },
    ));

    // --- Camera2d (overlay: composite sprite + UI) ---
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(LAYER_OVERLAY),
    ));

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
        CompositeSprite,
    ));

    // --- Assets ---
    let soul_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/characters/soul.glb"));
    let gltf_handle: Handle<Gltf> = asset_server.load("models/characters/soul.glb");
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
    let soul_shadow_material = soul_shadow_materials.add(SoulShadowMaterial::default());

    // --- Directional light (production-equivalent shadow setup) ---
    let sun_dir = topdown_sun_direction_world();
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 12_000.0,
            ..default()
        },
        Transform::from_translation(sun_dir * 360.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 120.0,
            maximum_distance: 500.0,
            ..default()
        }
        .build(),
        RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]),
    ));

    commands.insert_resource(TestAssets {
        soul_scene: soul_scene.clone(),
        face_atlas: face_atlas.clone(),
        white_pixel: white_pixel.clone(),
        gltf_handle,
        soul_shadow_material: soul_shadow_material.clone(),
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
                soul_shadow_material: &soul_shadow_material,
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

    // --- Right-side menu panel ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(MENU_WIDTH),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.04, 0.04, 0.82)),
            MenuPanel,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.92, 0.92)),
                MenuText,
            ));
        });

    // --- Hint shown when panel is collapsed ---
    commands.spawn((
        Text::new("[H] メニュー表示"),
        TextFont {
            font,
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
        Visibility::Hidden,
        MenuHint,
    ));
}

fn setup_world_map(mut commands: Commands, asset_server: Res<AssetServer>) {
    let grass = asset_server.load("textures/grass.png");
    let dirt = asset_server.load("textures/dirt.png");
    let river = asset_server.load("textures/river.png");
    let sand = asset_server.load("textures/sand_terrain.png");

    let terrain = generate_base_terrain_tiles(MAP_WIDTH, MAP_HEIGHT, SAND_WIDTH);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            let (texture, z) = match terrain[idx] {
                TerrainType::Grass => (grass.clone(), Z_MAP_GRASS),
                TerrainType::Dirt => (dirt.clone(), Z_MAP_DIRT),
                TerrainType::River => (river.clone(), Z_MAP),
                TerrainType::Sand => (sand.clone(), Z_MAP_SAND),
            };
            let pos = grid_to_world(x, y);
            commands.spawn((
                Sprite {
                    image: texture,
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, z),
                RenderLayers::layer(LAYER_2D),
                WorldMapTile,
            ));
        }
    }
}

// ─── Soul spawning ──────────────────────────────────────────────────────────

const MAX_SOULS: usize = 6;

struct SoulSpawnArgs<'a> {
    soul_scene: &'a Handle<Scene>,
    face_atlas: &'a Handle<Image>,
    white_pixel: &'a Handle<Image>,
    soul_shadow_material: &'a Handle<SoulShadowMaterial>,
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
    let soul_entity = entity.id();

    // Shadow proxy — matches production SoulShadowProxy3d spawn
    commands.spawn((
        SceneRoot(args.soul_scene.clone()),
        Transform::from_xyz(args.x, 0.0, args.z)
            .with_scale(Vec3::splat(SOUL_GLB_SCALE))
            .with_rotation(Quat::from_rotation_x(
                SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES.to_radians(),
            )),
        RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SOUL_SHADOW]),
        SoulShadowProxy3d { owner: soul_entity },
        SoulShadowConfig {
            shadow_mat: args.soul_shadow_material.clone(),
        },
    ));
}

// ─── GLB material replacement + animation setup observer ───────────────────

#[allow(clippy::too_many_arguments)]
fn on_soul_scene_ready(
    scene_ready: On<SceneInstanceReady>,
    q_configs: Query<&TestSoulConfig>,
    q_children: Query<&Children>,
    q_mesh_names: Query<&GltfMeshName>,
    q_names: Query<&Name>,
    q_meshes: Query<(), With<Mesh3d>>,
    q_transforms: Query<&Transform>,
    q_anim_players: Query<(), With<AnimationPlayer>>,
    assets: Option<Res<TestAssets>>,
    gltfs: Res<Assets<Gltf>>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let Ok(config) = q_configs.get(scene_ready.entity) else {
        return;
    };

    let render_layers = RenderLayers::layer(LAYER_3D);
    let mut anim_player_entity: Option<Entity> = None;

    for child in q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert(render_layers.clone());

        if q_anim_players.get(child).is_ok() {
            anim_player_entity = Some(child);
        }

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
                .insert((
                    MeshMaterial3d(config.face_mat.clone()),
                    NotShadowCaster,
                ));
            continue;
        }

        let is_body_mesh = matches!(mesh_name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010.SoulMat"));
        if is_body_mesh {
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((
                    MeshMaterial3d::<CharacterMaterial>(config.body_mat.clone()),
                    NotShadowCaster,
                ));
        }
    }

    // ── アニメーション設定 ──────────────────────────────────────────────────
    let Some(player_entity) = anim_player_entity else {
        return;
    };
    let Some(ref assets) = assets else {
        return;
    };
    let Some(gltf) = gltfs.get(&assets.gltf_handle) else {
        return;
    };

    let mut graph = AnimationGraph::new();
    let clips: Vec<(&'static str, AnimationNodeIndex)> = ANIM_CLIP_NAMES
        .iter()
        .filter_map(|name| {
            gltf.named_animations
                .get(*name)
                .cloned()
                .map(|clip| (*name, graph.add_clip(clip, 1.0, graph.root)))
        })
        .collect();

    if clips.is_empty() {
        return;
    }

    let graph_handle = anim_graphs.add(graph);
    commands.entity(player_entity).insert((
        AnimationGraphHandle(graph_handle),
        AnimationTransitions::new(),
    ));

    commands.entity(scene_ready.entity).insert(SoulAnimHandle {
        anim_player_entity: player_entity,
        clips,
        current_playing: usize::MAX,
    });
}

// ─── Shadow proxy GLB material replacement observer ──────────────────────────

fn on_shadow_scene_ready(
    scene_ready: On<SceneInstanceReady>,
    q_configs: Query<&SoulShadowConfig>,
    q_children: Query<&Children>,
    q_meshes: Query<(), With<Mesh3d>>,
    mut commands: Commands,
) {
    let Ok(config) = q_configs.get(scene_ready.entity) else {
        return;
    };

    let render_layers = RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SOUL_SHADOW]);

    for child in q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert((render_layers.clone(), NotShadowReceiver));
        entity_commands.remove::<NotShadowCaster>();

        if q_meshes.get(child).is_ok() {
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(config.shadow_mat.clone()));
        }
    }
}

// ─── Input handling ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<TestState>,
    mut elev: ResMut<TestElev>,
    mut commands: Commands,
    assets: Option<Res<TestAssets>>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    mut q_souls: Query<(
        Entity,
        &mut Transform,
        &TestSoulConfig,
        Option<&SelectedSoul>,
    )>,
    q_anim_handles: Query<&SoulAnimHandle>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    // [H] → メニュー表示切り替え
    if keys.just_pressed(KeyCode::KeyH) {
        state.menu_visible = !state.menu_visible;
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

    // [G] → all-different mode ([A] は PanCamera パン左に使用)
    if keys.just_pressed(KeyCode::KeyG) {
        state.face_mode = FaceMode::AllDifferent;
    }

    // [M] → cycle motion
    if keys.just_pressed(KeyCode::KeyM) {
        state.motion = state.motion.next();
    }

    // [Q] → cycle animation clip
    if keys.just_pressed(KeyCode::KeyQ) {
        let num_clips = q_anim_handles
            .iter()
            .next()
            .map(|h| h.clips.len())
            .unwrap_or(ANIM_CLIP_NAMES.len());
        if num_clips > 0 {
            state.anim_clip_idx = (state.anim_clip_idx + 1) % num_clips;
        }
    }

    // Shader params
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

    // [V] → 矢視方向切替 (TopDown → North → East → South → West)
    if keys.just_pressed(KeyCode::KeyV) {
        elev.dir = elev.dir.next();
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

    // Camera angle
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
                soul_shadow_material: &assets.soul_shadow_material,
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

    // [-] → remove last soul
    if keys.just_pressed(KeyCode::Minus) && state.soul_count > 1 {
        let mut candidates: Vec<_> = q_souls
            .iter()
            .map(|(e, _, cfg, sel)| (e, cfg.index, sel.is_some()))
            .collect();
        candidates.sort_by_key(|(_, idx, selected)| {
            (std::cmp::Reverse(*selected as u8), std::cmp::Reverse(*idx))
        });
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
        for &(entity, _, sel) in &sorted {
            if sel {
                commands.entity(entity).remove::<SelectedSoul>();
            }
        }
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

// ─── Animation application ─────────────────────────────────────────────────

fn apply_animation(
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

// ─── Shader parameter application ──────────────────────────────────────────

fn apply_shader_params(
    state: Res<TestState>,
    q_souls: Query<&TestSoulConfig, With<SelectedSoul>>,
    mut materials: ResMut<Assets<CharacterMaterial>>,
) {
    let Ok(config) = q_souls.single() else {
        return;
    };
    if let Some(mat) = materials.get_mut(&config.body_mat) {
        mat.params.ghost_alpha = state.ghost_alpha;
        mat.params.rim_strength = state.rim_strength;
        mat.params.posterize_steps = state.posterize_steps;
    }
}

// ─── Camera3d ↔ Camera2d 同期（ゲームの sync_camera3d_system 相当） ──────────

fn sync_test_camera3d(
    state: Res<TestState>,
    elev: Res<TestElev>,
    q_cam2d: Query<&Transform, (With<TestMainCamera>, Without<Camera3dRtt>)>,
    mut q_cam3d: Cam3dSyncQuery,
) {
    let Ok(cam2d) = q_cam2d.single() else {
        return;
    };
    let scene_z = -cam2d.translation.y; // 2D y → 3D z 変換

    for (mut cam3d, mut projection) in &mut q_cam3d {
        let soul_mid_y = SOUL_GLB_SCALE * 0.5;
        match elev.dir {
            TestElevDir::TopDown => {
                cam3d.translation.x = cam2d.translation.x;
                cam3d.translation.y = state.view_height;
                cam3d.translation.z = scene_z + state.z_offset;
                cam3d.rotation = elev.dir.camera_rotation(state.view_height, state.z_offset);
            }
            TestElevDir::North => {
                cam3d.translation.x = cam2d.translation.x;
                cam3d.translation.y = soul_mid_y;
                cam3d.translation.z = scene_z + ELEV_DISTANCE;
                cam3d.rotation = elev.dir.camera_rotation(state.view_height, state.z_offset);
            }
            TestElevDir::South => {
                cam3d.translation.x = cam2d.translation.x;
                cam3d.translation.y = soul_mid_y;
                cam3d.translation.z = scene_z - ELEV_DISTANCE;
                cam3d.rotation = elev.dir.camera_rotation(state.view_height, state.z_offset);
            }
            TestElevDir::East => {
                cam3d.translation.x = cam2d.translation.x + ELEV_DISTANCE;
                cam3d.translation.y = soul_mid_y;
                cam3d.translation.z = scene_z;
                cam3d.rotation = elev.dir.camera_rotation(state.view_height, state.z_offset);
            }
            TestElevDir::West => {
                cam3d.translation.x = cam2d.translation.x - ELEV_DISTANCE;
                cam3d.translation.y = soul_mid_y;
                cam3d.translation.z = scene_z;
                cam3d.rotation = elev.dir.camera_rotation(state.view_height, state.z_offset);
            }
        }
        cam3d.scale = Vec3::ONE;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = cam2d.scale.x;
        }
    }
}

// ─── Composite sprite サイズ更新 ──────────────────────────────────────────────

fn apply_composite_sprite(
    state: Res<TestState>,
    elev: Res<TestElev>,
    mut q_sprite: Query<&mut Sprite, With<CompositeSprite>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    if !state.is_changed() && !elev.is_changed() {
        return;
    }
    let Ok(mut sprite) = q_sprite.single_mut() else {
        return;
    };
    let Ok(win) = q_window.single() else {
        return;
    };
    let s = win.size();
    let comp = if elev.dir.is_top_down() {
        state.view_height.hypot(state.z_offset) / state.view_height
    } else {
        1.0
    };
    sprite.custom_size = Some(Vec2::new(s.x, s.y * comp));
}

// ─── Menu visibility ───────────────────────────────────────────────────────

fn apply_menu_visibility(
    state: Res<TestState>,
    mut q_panel: Query<&mut Visibility, With<MenuPanel>>,
    mut q_hint: Query<&mut Visibility, (With<MenuHint>, Without<MenuPanel>)>,
) {
    if !state.is_changed() {
        return;
    }
    if let Ok(mut vis) = q_panel.single_mut() {
        *vis = if state.menu_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut vis) = q_hint.single_mut() {
        *vis = if state.menu_visible {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

// ─── HUD / menu text update ────────────────────────────────────────────────

fn update_hud(
    state: Res<TestState>,
    elev: Res<TestElev>,
    q_selected: Query<(&TestSoulConfig, Option<&SoulAnimHandle>), With<SelectedSoul>>,
    q_players: Query<&AnimationPlayer>,
    mut q_text: Query<&mut Text, With<MenuText>>,
) {
    if !state.menu_visible {
        return;
    }
    let Ok(mut text) = q_text.single_mut() else {
        return;
    };

    let selected = q_selected.single().ok();
    let soul_label = selected
        .map(|(c, _)| format!("Soul#{}", c.index))
        .unwrap_or_else(|| "なし".to_string());
    let anim_handle = selected.and_then(|(_, h)| h);

    let anim_seek = anim_handle
        .and_then(|h| h.clips.get(state.anim_clip_idx))
        .map(|&(_, node)| {
            q_players
                .get(anim_handle.unwrap().anim_player_entity)
                .ok()
                .and_then(|p| p.animation(node))
                .map(|a| a.seek_time())
                .unwrap_or(0.0)
        })
        .unwrap_or(0.0);

    let mut s = String::with_capacity(1024);

    // ── ヘッダー ─────────────────────────────────────────────────────────────
    let _ = writeln!(s, "━━ Visual Test ━━━━━━━━━━━━━━");
    let _ = writeln!(
        s,
        " Souls:{}/{}   {soul_label}",
        state.soul_count, MAX_SOULS
    );
    let _ = writeln!(s, " [H] メニューを閉じる");

    // ── 表情 ──────────────────────────────────────────────────────────────────
    let _ = writeln!(s, "\n─ 表情  [1-6]  [G]:全体 ─────");
    for (i, expr) in FaceExpression::ALL.iter().enumerate() {
        let cur = matches!(state.face_mode, FaceMode::Single(e) if e == *expr);
        let mark = if cur { "►" } else { " " };
        let _ = writeln!(s, " {mark}[{}] {}", i + 1, expr.label());
    }
    let all_cur = matches!(state.face_mode, FaceMode::AllDifferent);
    let all_mark = if all_cur { "►" } else { " " };
    let _ = writeln!(s, " {all_mark}[G] 全表情モード");

    // ── アニメーション ──────────────────────────────────────────────────────────
    let _ = writeln!(s, "\n─ アニメーション  [Q]:次へ ───");
    let num_clips = anim_handle
        .map(|h| h.clips.len())
        .unwrap_or(ANIM_CLIP_NAMES.len());
    for i in 0..num_clips {
        let name = anim_handle
            .and_then(|h| h.clips.get(i))
            .map(|(n, _)| *n)
            .or_else(|| ANIM_CLIP_NAMES.get(i).copied())
            .unwrap_or("?");
        let mark = if i == state.anim_clip_idx { "►" } else { " " };
        if i == state.anim_clip_idx {
            let _ = writeln!(s, " {mark} {name}  ({anim_seek:.1}s)");
        } else {
            let _ = writeln!(s, " {mark} {name}");
        }
    }

    // ── Transform モーション ─────────────────────────────────────────────────
    let _ = writeln!(s, "\n─ Transform  [M]:次へ ────────");
    for mode in MotionMode::ALL {
        let mark = if mode == state.motion { "►" } else { " " };
        let _ = writeln!(s, " {mark} {}", mode.label());
    }

    // ── シェーダー ───────────────────────────────────────────────────────────
    let _ = writeln!(s, "\n─ シェーダー  [P]:reset ──────");
    let _ = writeln!(s, " ghost_alpha   {:>5.2}  [Z]/[X]", state.ghost_alpha);
    let _ = writeln!(s, " rim_strength  {:>5.2}  [C]/[F]", state.rim_strength);
    let _ = writeln!(s, " posterize     {:>5.1}  [B]/[N]", state.posterize_steps);

    // ── カメラ ────────────────────────────────────────────────────────────────
    let _ = writeln!(s, "\n─ カメラ ────────────────────────");
    let _ = writeln!(s, " [W/A/S/D]  パン");
    let _ = writeln!(s, " [スクロール]  ズーム");
    let _ = writeln!(s, " [V]        矢視切替");
    let _ = writeln!(s, " 方向:  {}", elev.dir.label());
    let _ = writeln!(s, "\n─ 仰角  [O]:reset ───────────");
    let elev_deg = state.z_offset.atan2(state.view_height).to_degrees();
    let _ = writeln!(s, " HEIGHT  {:>5.0}  [J]/[K]", state.view_height);
    let _ = writeln!(s, " OFFSET  {:>5.0}  [U]/[I]", state.z_offset);
    let _ = writeln!(s, " 仰角    {:>5.1}°", elev_deg);

    // ── Soul 管理 ────────────────────────────────────────────────────────────
    let _ = writeln!(s, "\n─ Soul管理 ───────────────────");
    let _ = writeln!(s, " [=]/[-]   追加 / 削除");
    let _ = writeln!(s, " [Tab]     選択切替");
    let _ = writeln!(s, " [R]       位置リセット");
    let _ = writeln!(s, " [←→↑↓]  移動");
    let _ = writeln!(s, " [Esc]     終了");

    **text = s;
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
