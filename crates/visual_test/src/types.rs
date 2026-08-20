use bevy::color::Srgba;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};

// ─── 定数 ────────────────────────────────────────────────────────────────────

pub const MENU_WIDTH: f32 = 270.0;

mod domain;
mod render;
mod state;
mod ui;

pub use domain::TestBuildingKind;
pub use render::{
    Cam2dQuery, Cam3dSyncQuery, Camera3dRtt, LocalRttComposite, LocalRttCompositeMaterial,
    RttCompositeParams, TestMainCamera, VisualTestRttRuntime,
};
pub use state::TestState;
pub use ui::{
    BTN_ACT, BTN_ACT_H, BTN_DEF, BTN_HOVER, BTN_PRESS, BuildSectionNode, DynamicTextKind, MenuHint,
    MenuPanel, VisualTestAction, WorldMapTile,
};
