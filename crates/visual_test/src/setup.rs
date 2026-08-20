use bevy::camera::ClearColorConfig;
use bevy::camera::visibility::RenderLayers;
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::ecs::system::SystemParam;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite_render::MeshMaterial2d;
use bevy::window::PrimaryWindow;
use hw_core::constants::{
    LAYER_2D, LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_OVERLAY, VIEW_HEIGHT, Z_OFFSET,
    Z_RTT_COMPOSITE, topdown_rtt_vertical_compensation, topdown_sun_direction_world,
};

use crate::types::*;

mod menu;
mod scene;

pub use scene::{SceneRenderAssets, setup_scene};

use menu::spawn_menu_ui;
