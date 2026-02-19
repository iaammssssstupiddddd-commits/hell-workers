//! UI Theme - UiTheme resource for centralized styling
//!
//! All UI colors, sizes, typography, and spacing are defined here as a Resource.

use bevy::prelude::*;

// ============================================================
// Theme sub-structures
// ============================================================

pub struct ThemeColors {
    // Base Palette
    pub bg_surface: Color,
    pub bg_elevated: Color,
    pub bg_overlay: Color,

    // Accents
    pub accent_ember: Color,
    pub accent_ember_bright: Color,
    pub accent_soul: Color,
    pub accent_soul_bright: Color,
    pub accent_sulfur: Color,

    // Semantic Mapping
    pub text_primary_semantic: Color,
    pub text_secondary_semantic: Color,
    pub text_accent_semantic: Color,
    pub border_default: Color,
    pub border_accent: Color,

    // Status Colors
    pub status_healthy: Color,
    pub status_warning: Color,
    pub status_danger: Color,
    pub status_info: Color,

    // Panel Accents
    pub panel_accent_entity_list: Color,
    pub panel_accent_info_panel: Color,
    pub panel_accent_control_bar: Color,
    pub panel_accent_time_control: Color,

    // Text
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_accent: Color,
    pub text_muted: Color,
    pub header_text: Color,
    pub empty_text: Color,

    // Gender
    pub male: Color,
    pub female: Color,

    // Task/Work type
    pub idle: Color,
    pub chop: Color,
    pub mine: Color,
    pub gather_default: Color,
    pub haul: Color,
    pub build: Color,
    pub haul_to_bp: Color,
    pub water: Color,

    // Stress indicators
    pub stress_high: Color,
    pub stress_medium: Color,
    pub stress_icon: Color,

    // Fatigue indicators
    pub fatigue_icon: Color,
    pub fatigue_text: Color,

    // Buttons
    pub button_default: Color,
    pub button_hover: Color,
    pub button_pressed: Color,

    // List items
    pub list_item_default: Color,
    pub list_item_hover: Color,
    pub list_item_selected: Color,
    pub list_item_selected_hover: Color,
    pub list_selection_border: Color,

    // Familiar header
    pub familiar_header_hover: Color,
    pub familiar_header_selected: Color,
    pub familiar_header_selected_hover: Color,

    // UI elements
    pub fold_button_bg: Color,
    pub familiar_button_bg: Color,
    pub section_toggle_pressed: Color,
    pub overlay_row_bg: Color,

    // Surface backgrounds
    pub submenu_bg: Color,
    pub tooltip_bg: Color,
    pub tooltip_border: Color,
    pub dialog_bg: Color,
    pub dialog_border: Color,

    // Time control panel
    pub time_control_bg: Color,
    pub time_control_border: Color,
    pub time_control_separator: Color,
    pub speed_button_active: Color,
    pub task_high_warning: Color,
}

pub struct PanelGradient {
    pub top: Color,
    pub bottom: Color,
}

pub struct PanelThemes {
    pub entity_list: PanelGradient,
    pub info_panel: PanelGradient,
    pub bottom_bar: PanelGradient,
}

pub struct ThemeTypography {
    pub font_size_xs: f32,
    pub font_size_sm: f32,
    pub font_size_base: f32,
    pub font_size_md: f32,
    pub font_size_lg: f32,
    pub font_size_xl: f32,

    pub font_size_title: f32,
    pub font_size_header: f32,
    pub font_size_item: f32,
    pub font_size_small: f32,
    pub font_size_clock: f32,
    pub font_size_status: f32,
    /// Dialog small font size (previously constants::FONT_SIZE_SMALL)
    pub font_size_dialog_small: f32,
    /// Dialog tiny font size (previously constants::FONT_SIZE_TINY)
    pub font_size_dialog_tiny: f32,
}

pub struct ThemeSpacing {
    pub margin_small: f32,
    pub margin_medium: f32,
    pub margin_large: f32,
    pub text_left_padding: f32,
    pub panel_padding: f32,
    pub panel_margin_x: f32,
    pub panel_top: f32,
    pub bottom_bar_height: f32,
    pub bottom_bar_padding: f32,
}

pub struct ThemeSizes {
    // Panel Frame
    pub panel_border_width: f32,
    pub panel_corner_radius: f32,

    // Entity List Constraints
    pub entity_list_min_width: f32,
    pub entity_list_max_width: f32,

    // Info Panel Constraints
    pub info_panel_min_width: f32,
    pub info_panel_max_width: f32,

    // Tooltip
    pub tooltip_border_width: f32,
    pub tooltip_corner_radius: f32,
    pub tooltip_min_width: f32,
    pub tooltip_max_width: f32,
    pub tooltip_padding: f32,

    pub header_height: f32,
    pub soul_item_height: f32,
    pub icon_size: f32,
    pub fold_icon_size: f32,
    pub fold_button_size: f32,
    pub familiar_section_margin_top: f32,
    pub squad_member_left_margin: f32,
    pub empty_squad_left_margin: f32,
    pub list_selection_border_width: f32,
    pub entity_list_panel_width: f32,
    pub entity_list_max_height_percent: f32,
    pub info_panel_width: f32,
    pub submenu_width: f32,
    pub submenu_left_architect: f32,
    pub submenu_left_zones: f32,
    pub submenu_left_orders: f32,
    pub submenu_left_dream: f32,
    pub time_control_top: f32,
    pub fps_left: f32,
    pub fps_top: f32,
}

// ============================================================
// UiTheme Resource
// ============================================================

#[derive(Resource)]
pub struct UiTheme {
    pub colors: ThemeColors,
    pub typography: ThemeTypography,
    pub spacing: ThemeSpacing,
    pub sizes: ThemeSizes,
    pub panels: PanelThemes,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            colors: ThemeColors {
                // Base Palette (Dark Purple/Hell Theme)
                bg_surface: Color::srgb(0.08, 0.03, 0.06),
                bg_elevated: Color::srgb(0.12, 0.05, 0.10),
                bg_overlay: Color::srgb(0.18, 0.08, 0.14),

                // Embers (Orange Accent)
                accent_ember: Color::srgb(0.8, 0.4, 0.1),
                accent_ember_bright: Color::srgb(1.0, 0.6, 0.2),

                // Soul (Blue/Cyan Accent)
                accent_soul: Color::srgb(0.3, 0.5, 0.8),
                accent_soul_bright: Color::srgb(0.5, 0.7, 1.0),

                // Sulfur (Yellow Warning)
                accent_sulfur: Color::srgb(0.8, 0.7, 0.2),

                // Semantic Mapping
                text_primary_semantic: Color::srgb(0.9, 0.9, 0.92),
                text_secondary_semantic: Color::srgb(0.5, 0.5, 0.55),
                text_accent_semantic: Color::srgb(1.0, 0.6, 0.2),
                border_default: Color::srgb(0.25, 0.25, 0.3),
                border_accent: Color::srgb(0.8, 0.4, 0.1),

                // Status Colors
                status_healthy: Color::srgb(0.3, 0.8, 0.4),
                status_warning: Color::srgb(0.8, 0.7, 0.2),
                status_danger: Color::srgb(0.9, 0.2, 0.1),
                status_info: Color::srgb(0.3, 0.5, 0.8),

                // Panel Accents
                panel_accent_entity_list: Color::srgb(0.3, 0.5, 0.8),
                panel_accent_info_panel: Color::srgb(0.7, 0.3, 0.6),
                panel_accent_control_bar: Color::srgb(0.8, 0.3, 0.2),
                panel_accent_time_control: Color::srgb(0.8, 0.7, 0.3),

                // Legacy Colors (Keeping for compatibility)
                text_primary: Color::WHITE,
                text_secondary: Color::srgb(0.7, 0.7, 0.7),
                text_accent: Color::srgb(0.0, 1.0, 1.0),
                text_muted: Color::srgba(1.0, 1.0, 1.0, 0.3),
                header_text: Color::srgb(0.8, 0.8, 1.0),
                empty_text: Color::srgb(0.5, 0.5, 0.5),

                // Gender
                male: Color::srgb(0.4, 0.7, 1.0),
                female: Color::srgb(1.0, 0.5, 0.7),

                // Task/Work type
                idle: Color::srgb(0.6, 0.6, 0.6),
                chop: Color::srgb(0.6, 0.4, 0.2),
                mine: Color::srgb(0.7, 0.7, 0.7),
                gather_default: Color::srgb(1.0, 0.7, 0.3),
                haul: Color::srgb(0.5, 1.0, 0.5),
                build: Color::srgb(0.8, 0.6, 0.2),
                haul_to_bp: Color::srgb(0.8, 0.8, 0.3),
                water: Color::srgb(0.3, 0.5, 1.0),

                // Stress indicators
                stress_high: Color::srgb(1.0, 0.0, 0.0),
                stress_medium: Color::srgb(1.0, 0.5, 0.0),
                stress_icon: Color::srgb(1.0, 0.9, 0.2),

                // Fatigue indicators
                fatigue_icon: Color::srgb(0.6, 0.6, 1.0),
                fatigue_text: Color::srgb(0.7, 0.7, 1.0),

                // Buttons (Legacy mapping to new interactive colors + alpha for some)
                button_default: Color::srgb(0.25, 0.25, 0.3),
                button_hover: Color::srgb(0.35, 0.15, 0.28),
                button_pressed: Color::srgb(0.8, 0.4, 0.1),

                // List items
                list_item_default: Color::NONE,
                list_item_hover: Color::srgba(0.12, 0.05, 0.10, 0.55), // bg_elevated (hover)
                list_item_selected: Color::srgba(0.12, 0.05, 0.10, 0.74), // bg_elevated (selected)
                list_item_selected_hover: Color::srgba(0.18, 0.08, 0.14, 0.82), // bg_overlay tint
                list_selection_border: Color::srgba(0.8, 0.4, 0.1, 0.95), // border_accent

                // Familiar header
                familiar_header_hover: Color::srgba(0.28, 0.28, 0.5, 0.75),
                familiar_header_selected: Color::srgba(0.24, 0.4, 0.62, 0.82),
                familiar_header_selected_hover: Color::srgba(0.28, 0.45, 0.67, 0.9),

                // UI elements
                fold_button_bg: Color::srgba(0.3, 0.3, 0.5, 0.6),
                familiar_button_bg: Color::srgba(0.2, 0.2, 0.4, 0.6),
                section_toggle_pressed: Color::srgba(0.5, 0.5, 0.5, 0.8),
                overlay_row_bg: Color::srgba(1.0, 1.0, 1.0, 0.05),

                // Surface backgrounds
                submenu_bg: Color::srgba(0.12, 0.05, 0.10, 0.95), // bg_elevated
                tooltip_bg: Color::srgba(0.18, 0.08, 0.14, 0.95), // bg_overlay
                tooltip_border: Color::srgb(0.25, 0.25, 0.3),     // border_default
                dialog_bg: Color::srgba(0.08, 0.03, 0.06, 0.98),  // bg_surface
                dialog_border: Color::srgb(0.8, 0.4, 0.1),        // border_accent

                // Time control panel
                time_control_bg: Color::srgba(0.05, 0.03, 0.08, 0.80),
                time_control_border: Color::srgba(0.4, 0.25, 0.35, 0.6),
                time_control_separator: Color::srgba(0.4, 0.3, 0.35, 0.4),
                speed_button_active: Color::srgba(0.5, 0.25, 0.05, 0.7),
                task_high_warning: Color::srgb(0.95, 0.35, 0.2),
            },
            typography: ThemeTypography {
                // New Modular Scale
                font_size_xs: 9.0,
                font_size_sm: 11.0,
                font_size_base: 13.0,
                font_size_md: 15.0,
                font_size_lg: 18.0,
                font_size_xl: 22.0,

                // Legacy
                font_size_title: 18.0,
                font_size_header: 14.0,
                font_size_item: 12.0,
                font_size_small: 10.0,
                font_size_clock: 22.0,
                font_size_status: 16.0,
                font_size_dialog_small: 14.0,
                font_size_dialog_tiny: 10.0,
            },
            spacing: ThemeSpacing {
                margin_small: 2.0,
                margin_medium: 4.0,
                margin_large: 6.0,
                text_left_padding: 4.0,
                panel_padding: 10.0,
                panel_margin_x: 20.0,
                panel_top: 170.0,
                bottom_bar_height: 50.0,
                bottom_bar_padding: 5.0,
            },
            sizes: ThemeSizes {
                // Panel Frame
                panel_border_width: 1.0,
                panel_corner_radius: 4.0,

                // Entity List Constraints
                entity_list_min_width: 200.0,
                entity_list_max_width: 450.0,

                // Info Panel Constraints
                info_panel_min_width: 200.0,
                info_panel_max_width: 400.0,

                // Tooltip
                tooltip_border_width: 1.0,
                tooltip_corner_radius: 3.0,
                tooltip_min_width: 220.0,
                tooltip_max_width: 280.0,
                tooltip_padding: 8.0,

                // Legacy
                header_height: 24.0,
                soul_item_height: 20.0,
                icon_size: 16.0,
                fold_icon_size: 12.0,
                fold_button_size: 20.0,
                familiar_section_margin_top: 4.0,
                squad_member_left_margin: 15.0,
                empty_squad_left_margin: 15.0,
                list_selection_border_width: 3.0,
                entity_list_panel_width: 300.0,
                entity_list_max_height_percent: 70.0,
                info_panel_width: 260.0, // Increased from 200.0
                submenu_width: 120.0,
                submenu_left_architect: 0.0,
                submenu_left_zones: 110.0,
                submenu_left_orders: 220.0,
                submenu_left_dream: 330.0,
                time_control_top: 20.0,
                fps_left: 20.0,
                fps_top: 20.0,
            },
            panels: PanelThemes {
                entity_list: PanelGradient {
                    top: Color::srgb(0.12, 0.05, 0.10),    // bg_elevated
                    bottom: Color::srgb(0.08, 0.03, 0.06), // bg_surface
                },
                info_panel: PanelGradient {
                    top: Color::srgb(0.12, 0.05, 0.10),    // bg_elevated
                    bottom: Color::srgb(0.08, 0.03, 0.06), // bg_surface
                },
                bottom_bar: PanelGradient {
                    top: Color::srgb(0.12, 0.05, 0.10),    // bg_elevated
                    bottom: Color::srgb(0.08, 0.03, 0.06), // bg_surface
                },
            },
        }
    }
}
