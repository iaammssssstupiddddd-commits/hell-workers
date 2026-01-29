//! UI Theme constants - colors, sizes, and visual styling
//!
//! Centralized theme definitions for consistent UI appearance.

use bevy::prelude::*;

// ============================================================
// Size constants
// ============================================================

pub const HEADER_HEIGHT: f32 = 24.0;
pub const SOUL_ITEM_HEIGHT: f32 = 20.0;
pub const ICON_SIZE: f32 = 16.0;
pub const FOLD_ICON_SIZE: f32 = 12.0;
pub const FOLD_BUTTON_SIZE: f32 = 20.0;
pub const FAMILIAR_SECTION_MARGIN_TOP: f32 = 4.0;
pub const SQUAD_MEMBER_LEFT_MARGIN: f32 = 15.0;
pub const EMPTY_SQUAD_LEFT_MARGIN: f32 = 15.0;

// ============================================================
// Margin constants
// ============================================================

pub const MARGIN_SMALL: f32 = 2.0;
pub const MARGIN_MEDIUM: f32 = 4.0;
pub const MARGIN_LARGE: f32 = 6.0;
pub const TEXT_LEFT_PADDING: f32 = 4.0;

// ============================================================
// Font sizes
// ============================================================

pub const FONT_SIZE_HEADER: f32 = 14.0;
pub const FONT_SIZE_ITEM: f32 = 12.0;
pub const FONT_SIZE_SMALL: f32 = 10.0;

// ============================================================
// Color constants
// ============================================================

// Gender colors
pub const COLOR_MALE: Color = Color::srgb(0.4, 0.7, 1.0);
pub const COLOR_FEMALE: Color = Color::srgb(1.0, 0.5, 0.7);

// Task/Work type colors
pub const COLOR_IDLE: Color = Color::srgb(0.6, 0.6, 0.6);
pub const COLOR_CHOP: Color = Color::srgb(0.6, 0.4, 0.2);
pub const COLOR_MINE: Color = Color::srgb(0.7, 0.7, 0.7);
pub const COLOR_GATHER_DEFAULT: Color = Color::srgb(1.0, 0.7, 0.3);
pub const COLOR_HAUL: Color = Color::srgb(0.5, 1.0, 0.5);
pub const COLOR_BUILD: Color = Color::srgb(0.8, 0.6, 0.2);
pub const COLOR_HAUL_TO_BP: Color = Color::srgb(0.8, 0.8, 0.3);
pub const COLOR_WATER: Color = Color::srgb(0.3, 0.5, 1.0);

// Stress indicator colors
pub const COLOR_STRESS_HIGH: Color = Color::srgb(1.0, 0.0, 0.0);
pub const COLOR_STRESS_MEDIUM: Color = Color::srgb(1.0, 0.5, 0.0);

// Fatigue indicator colors
pub const COLOR_FATIGUE_ICON: Color = Color::srgb(0.6, 0.6, 1.0);
pub const COLOR_FATIGUE_TEXT: Color = Color::srgb(0.7, 0.7, 1.0);

// UI element colors
pub const COLOR_STRESS_ICON: Color = Color::srgb(1.0, 0.9, 0.2);
pub const COLOR_HEADER_TEXT: Color = Color::srgb(0.8, 0.8, 1.0);
pub const COLOR_EMPTY_TEXT: Color = Color::srgb(0.5, 0.5, 0.5);
pub const COLOR_FOLD_BUTTON_BG: Color = Color::srgba(0.3, 0.3, 0.5, 0.6);
pub const COLOR_FAMILIAR_BUTTON_BG: Color = Color::srgba(0.2, 0.2, 0.4, 0.6);
pub const COLOR_SECTION_TOGGLE_PRESSED: Color = Color::srgba(0.5, 0.5, 0.5, 0.8);
