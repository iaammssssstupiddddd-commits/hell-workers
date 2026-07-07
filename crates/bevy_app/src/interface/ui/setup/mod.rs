use bevy::prelude::*;
use hw_ui::setup::{setup_ui as hwui_setup_ui, SettingsPanelInitial, SetupUiParams};

pub fn setup_ui(
    commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<hw_ui::theme::UiTheme>,
    ui_nodes: ResMut<hw_ui::components::UiNodeRegistry>,
    info_panel_nodes: ResMut<hw_ui::components::InfoPanelNodes>,
    settings: Res<hw_core::GameSettings>,
) {
    let theme_ref = &theme;
    let settings_initial = SettingsPanelInitial {
        ui_scale: settings.ui_scale,
        camera_pan_speed: settings.camera_pan_speed,
        camera_mouse_pan_enabled: settings.camera_mouse_pan_enabled,
        default_time_speed: settings.default_time_speed,
        debug_gizmos_enabled: settings.debug_gizmos_enabled,
        fps_display_enabled: settings.fps_display_enabled,
    };

    hwui_setup_ui(
        commands,
        SetupUiParams {
            game_assets: &*game_assets,
            theme: theme_ref,
            ui_nodes,
            info_panel_nodes,
            settings_initial,
        },
        |commands, info_slot, _overlay_slot, ui_nodes, info_panel_nodes| {
            crate::interface::ui::panels::spawn_info_panel_ui(
                commands,
                &*game_assets,
                theme_ref,
                info_slot,
                ui_nodes,
                info_panel_nodes,
            );
        },
        |commands, overlay_slot| {
            crate::interface::ui::vignette::spawn_vignette_ui(commands, overlay_slot);
        },
    );
}
