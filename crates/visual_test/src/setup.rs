use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::ecs::system::SystemParam;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite_render::MeshMaterial2d;
use bevy::window::PrimaryWindow;
use hw_core::constants::{
    LAYER_2D, LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_MASK, LAYER_3D_SOUL_SHADOW,
    LAYER_OVERLAY, VIEW_HEIGHT, Z_OFFSET, Z_RTT_COMPOSITE, topdown_rtt_vertical_compensation,
    topdown_sun_direction_world,
};
use hw_visual::{CharacterMaterial, SoulMaskMaterial, SoulShadowMaterial};

use crate::soul::{SoulRebuildEntities, blob_shadow_outline, rebuild_soul_test_layout};
use crate::types::*;

mod menu;
mod scene;

pub use scene::{SceneRenderAssets, setup_scene};

use menu::spawn_menu_ui;
