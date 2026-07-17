use bevy::color::Srgba;
use bevy::ecs::system::SystemParam;
use bevy::gltf::Gltf;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};
use hw_core::constants::{VIEW_HEIGHT, Z_OFFSET};
use hw_visual::visual3d::{SoulMaskProxy3d, SoulShadowProxy3d};
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

mod domain;
mod render;
mod state;
mod ui;

pub use domain::{
    AppMode, FaceExpression, MotionMode, SoulLayout, TestBuildingKind, TestElev, TestElevDir,
};
pub use render::{
    AnimPlayerQuery, Cam2dQuery, Cam3dSyncQuery, Camera3dRtt, Camera3dSoulMaskTest,
    LocalRttComposite, LocalRttCompositeMaterial, RttCompositeParams, SoulAnimHandle,
    SoulBlobShadowProxy3d, SoulLayoutEntities, SoulMaskConfig, SoulShadowConfig, TestAssets,
    TestMainCamera, TestSoulConfig, face_uv_offset, face_uv_scale,
};
pub use state::{FaceMode, TestState};
pub use ui::{
    BTN_ACT, BTN_ACT_H, BTN_DEF, BTN_HOVER, BTN_PRESS, BuildSectionNode, DynamicTextKind, MenuHint,
    MenuPanel, SelectedSoul, SoulSectionNode, VisualTestAction, WorldMapTile,
};
