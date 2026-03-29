use bevy::color::Srgba;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};
use hw_core::constants::{VIEW_HEIGHT, Z_OFFSET};
use hw_visual::{CharacterMaterial, SoulMaskMaterial, SoulShadowMaterial};

// ─── 定数 ────────────────────────────────────────────────────────────────────

pub const SOUL_SPACING: f32 = hw_core::constants::SOUL_GLB_SCALE * 2.5;
pub const MENU_WIDTH: f32 = 270.0;
pub const MAX_SOULS: usize = 6;
pub const ELEV_DISTANCE: f32 = 200.0;
pub const ANIM_CLIP_NAMES: &[&str] = &[
    "Idle",
    "Walk",
    "Work",
    "Carry",
    "Fear",
    "Exhausted",
    "WalkLeft",
    "WalkRight",
];
pub const DEFAULT_GHOST_ALPHA: f32 = 1.0;
pub const DEFAULT_RIM_STRENGTH: f32 = 0.28;
pub const DEFAULT_POSTERIZE_STEPS: f32 = 4.0;

// ─── 顔アトラス UV ────────────────────────────────────────────────────────────

const ATLAS_COLS: f32 = 3.0;
const ATLAS_ROWS: f32 = 2.0;
const CELL_PX: f32 = 256.0;
const CROP_PX: f32 = 152.0;
const MAG: f32 = 1.4;
const CROP_OX: f32 = 24.0;
const CROP_OY: f32 = 32.0;

pub fn face_uv_scale() -> Vec2 {
    Vec2::new(
        CROP_PX / MAG / CELL_PX / ATLAS_COLS,
        CROP_PX / MAG / CELL_PX / ATLAS_ROWS,
    )
}

pub fn face_uv_offset(col: f32, row: f32) -> Vec2 {
    let adj = (CROP_PX - CROP_PX / MAG) * 0.5;
    Vec2::new(
        (col * CELL_PX + CROP_OX + adj) / (CELL_PX * ATLAS_COLS),
        (row * CELL_PX + CROP_OY + adj) / (CELL_PX * ATLAS_ROWS),
    )
}

// ─── RtT 合成マテリアル ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RttCompositeParams {
    pub pixel_size: Vec2,
    pub mask_radius_px: f32,
    pub mask_feather: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct LocalRttCompositeMaterial {
    #[uniform(0)]
    pub params: RttCompositeParams,
    #[texture(1)]
    #[sampler(2)]
    pub scene_texture: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub soul_mask_texture: Handle<Image>,
}

impl Material2d for LocalRttCompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rtt_composite_material.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

// ─── マーカーコンポーネント ───────────────────────────────────────────────────

#[derive(Component)]
pub struct LocalRttComposite;
#[derive(Component)]
pub struct Camera3dSoulMaskTest;
#[derive(Component)]
pub struct Camera3dRtt;
#[derive(Component)]
pub struct TestMainCamera;

// ─── 表情 ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FaceExpression {
    #[default]
    Normal,
    Fear,
    Exhausted,
    Concentration,
    Happy,
    Sleep,
}

impl FaceExpression {
    pub const ALL: [Self; 6] = [
        Self::Normal,
        Self::Fear,
        Self::Exhausted,
        Self::Concentration,
        Self::Happy,
        Self::Sleep,
    ];

    pub fn uv_offset(self) -> Vec2 {
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

    pub fn label(self) -> &'static str {
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

// ─── モーション ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionMode {
    #[default]
    Idle,
    FloatingBob,
    Sleeping,
    Resting,
    Escaping,
    Dancing,
}

impl MotionMode {
    pub const ALL: [Self; 6] = [
        Self::Idle,
        Self::FloatingBob,
        Self::Sleeping,
        Self::Resting,
        Self::Escaping,
        Self::Dancing,
    ];

    pub fn next(self) -> Self {
        match self {
            Self::Idle => Self::FloatingBob,
            Self::FloatingBob => Self::Sleeping,
            Self::Sleeping => Self::Resting,
            Self::Resting => Self::Escaping,
            Self::Escaping => Self::Dancing,
            Self::Dancing => Self::Idle,
        }
    }

    pub fn label(self) -> &'static str {
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

// ─── 矢視方向 ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestElevDir {
    #[default]
    TopDown,
    North,
    East,
    South,
    West,
}

impl TestElevDir {
    pub fn next(self) -> Self {
        match self {
            Self::TopDown => Self::North,
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::TopDown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TopDown => "TopDown",
            Self::North => "North",
            Self::East => "East",
            Self::South => "South",
            Self::West => "West",
        }
    }

    pub fn is_top_down(self) -> bool {
        self == Self::TopDown
    }

    pub fn camera_rotation(self, view_height: f32, z_offset: f32) -> Quat {
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

#[derive(Resource, Default)]
pub struct TestElev {
    pub dir: TestElevDir,
}

// ─── コンポーネント / リソース ────────────────────────────────────────────────

#[derive(Component)]
pub struct TestSoulConfig {
    pub face_mat: Handle<CharacterMaterial>,
    pub body_mat: Handle<CharacterMaterial>,
    pub index: usize,
}

#[derive(Component)]
pub struct SelectedSoul;
#[derive(Component)]
pub struct MenuPanel;
#[derive(Component)]
pub struct MenuHint;
#[derive(Component)]
pub struct WorldMapTile;

// ─── パネルボタン ─────────────────────────────────────────────────────────────

/// ソウルモード専用セクション。モード切替で Node::display を制御。
#[derive(Component)]
pub struct SoulSectionNode;

/// ビルドモード専用セクション。モード切替で Node::display を制御。
#[derive(Component)]
pub struct BuildSectionNode;

/// パネル内の動的テキスト。update_dynamic_texts で値を一括更新。
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DynamicTextKind {
    ViewDir,
    Height,
    Offset,
    Ghost,
    Rim,
    Posterize,
    CursorPos,
}

/// パネルボタンアクション。Changed<Interaction> ハンドラで処理する。
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VisualTestAction {
    SetMode(AppMode),
    // カメラ
    NextView,
    HeightDown,
    HeightUp,
    OffsetDown,
    OffsetUp,
    ResetElevation,
    // Soul
    SetFace(FaceExpression),
    SetFaceAll,
    SetAnimation(usize),
    SetMotion(MotionMode),
    GhostDown,
    GhostUp,
    RimDown,
    RimUp,
    PosterizeDown,
    PosterizeUp,
    ResetShader,
    AddSoul,
    RemoveSoul,
    SelectNextSoul,
    ResetSoulPos,
    // Build
    SetBuildingKind(TestBuildingKind),
    PlaceOrRemove,
    RemoveAllBuildings,
}

// ─── ボタンカラー定数 ─────────────────────────────────────────────────────────
pub const BTN_DEF: Color = Color::Srgba(Srgba::new(0.25, 0.25, 0.30, 1.0));
pub const BTN_HOVER: Color = Color::Srgba(Srgba::new(0.35, 0.15, 0.28, 1.0));
pub const BTN_PRESS: Color = Color::Srgba(Srgba::new(0.60, 0.30, 0.08, 1.0));
pub const BTN_ACT: Color = Color::Srgba(Srgba::new(0.80, 0.40, 0.10, 1.0));
pub const BTN_ACT_H: Color = Color::Srgba(Srgba::new(0.90, 0.50, 0.20, 1.0));

/// ビジュアルテストの操作モード。[Space] で切替。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppMode {
    #[default]
    Soul,
    Build,
}

impl AppMode {
    pub fn next(self) -> Self {
        match self {
            Self::Soul => Self::Build,
            Self::Build => Self::Soul,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Soul => "SOUL",
            Self::Build => "BUILD",
        }
    }
}

/// ビジュアルテスト内で建築物種別を選択するためのローカル enum。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestBuildingKind {
    #[default]
    Wall,
    Door,
    Floor,
    Tank,
    MudMixer,
    RestArea,
    Bridge,
    SandPile,
    BonePile,
    WheelbarrowParking,
    SoulSpa,
}

impl TestBuildingKind {
    pub const ALL: [Self; 11] = [
        Self::Wall,
        Self::Door,
        Self::Floor,
        Self::Tank,
        Self::MudMixer,
        Self::RestArea,
        Self::Bridge,
        Self::SandPile,
        Self::BonePile,
        Self::WheelbarrowParking,
        Self::SoulSpa,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Door => "Door",
            Self::Floor => "Floor",
            Self::Tank => "Tank",
            Self::MudMixer => "MudMixer",
            Self::RestArea => "RestArea",
            Self::Bridge => "Bridge",
            Self::SandPile => "SandPile",
            Self::BonePile => "BonePile",
            Self::WheelbarrowParking => "WheelbarrowParking",
            Self::SoulSpa => "SoulSpa",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Component)]
pub struct SoulShadowConfig {
    pub shadow_mat: Handle<SoulShadowMaterial>,
}

#[derive(Component)]
pub struct SoulMaskConfig {
    pub mask_mat: Handle<SoulMaskMaterial>,
}

#[derive(Component)]
pub struct SoulAnimHandle {
    pub anim_player_entity: Entity,
    pub clips: Vec<(&'static str, AnimationNodeIndex)>,
    pub current_playing: usize,
}

#[derive(Resource)]
pub struct TestAssets {
    pub soul_scene: Handle<Scene>,
    pub face_atlas: Handle<Image>,
    pub white_pixel: Handle<Image>,
    pub gltf_handle: Handle<Gltf>,
    pub soul_shadow_material: Handle<SoulShadowMaterial>,
    pub soul_mask_material: Handle<SoulMaskMaterial>,
}

#[derive(Clone, Copy, Debug)]
pub enum FaceMode {
    Single(FaceExpression),
    AllDifferent,
}

#[derive(Resource)]
pub struct TestState {
    pub mode: AppMode,
    pub face_mode: FaceMode,
    pub motion: MotionMode,
    pub soul_count: usize,
    pub next_index: usize,
    pub anim_clip_idx: usize,
    pub view_height: f32,
    pub z_offset: f32,
    pub ghost_alpha: f32,
    pub rim_strength: f32,
    pub posterize_steps: f32,
    pub menu_visible: bool,
    pub building_kind: TestBuildingKind,
    pub building_cursor: (i32, i32),
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            mode: AppMode::Soul,
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
            building_kind: TestBuildingKind::Wall,
            building_cursor: (50, 50),
        }
    }
}

// ─── クエリ型エイリアス ───────────────────────────────────────────────────────

pub type AnimPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut AnimationPlayer,
        &'static mut AnimationTransitions,
    ),
>;

pub type Cam3dSyncQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Projection),
    Or<(With<Camera3dRtt>, With<Camera3dSoulMaskTest>)>,
>;

pub type Cam2dQuery<'w, 's> = Query<
    'w,
    's,
    &'static Transform,
    (
        With<TestMainCamera>,
        Without<Camera3dRtt>,
        Without<Camera3dSoulMaskTest>,
    ),
>;
