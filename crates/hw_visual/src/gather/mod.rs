//! 伐採・採掘ビジュアルシステム

mod components;
mod resource_highlight;
mod worker_indicator;

use bevy::prelude::Color;

pub use resource_highlight::{
    COLOR_DESIGNATED_TINT, COLOR_WORKING_TINT,
    attach_resource_visual_system, update_resource_visual_system, cleanup_resource_visual_system,
};
pub use worker_indicator::{spawn_gather_indicators_system, update_gather_indicators_system};

pub const COLOR_CHOP_ICON: Color = Color::srgb(0.4, 0.9, 0.3);
pub const COLOR_MINE_ICON: Color = Color::srgb(0.7, 0.7, 0.8);

pub const GATHER_ICON_SIZE: f32 = 16.0;
pub const GATHER_ICON_Y_OFFSET: f32 = 32.0;
pub const GATHER_ICON_BOB_SPEED: f32 = 5.0;
pub const GATHER_ICON_BOB_AMPLITUDE: f32 = 2.5;
