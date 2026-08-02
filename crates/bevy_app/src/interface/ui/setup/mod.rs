use bevy::prelude::*;
use hw_ui::setup::{SettingsPanelInitial, SetupUiParams, setup_ui as hwui_setup_ui};

pub fn setup_ui(
    commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<hw_ui::theme::UiTheme>,
    ui_nodes: ResMut<hw_ui::components::UiNodeRegistry>,
    info_panel_nodes: ResMut<hw_ui::components::InfoPanelNodes>,
    settings: Res<hw_core::GameSettings>,
    help_content: Res<hw_ui::help::HelpPanelContent>,
) {
    let theme_ref = &theme;
    let settings_initial = SettingsPanelInitial {
        ui_scale: settings.ui_scale,
        camera_pan_speed: settings.camera_pan_speed,
        camera_mouse_pan_enabled: settings.camera_mouse_pan_enabled,
        default_time_speed: settings.default_time_speed,
        debug_gizmos_enabled: settings.debug_gizmos_enabled,
        fps_display_enabled: settings.fps_display_enabled,
        power_priority_enabled: settings.power_priority_enabled,
    };
    let help_chrome = crate::interface::ui::help_content::build_help_panel_chrome()
        .expect("validated Help chrome");

    hwui_setup_ui(
        commands,
        SetupUiParams {
            game_assets: &*game_assets,
            theme: theme_ref,
            ui_nodes,
            info_panel_nodes,
            settings_initial,
            help_content: &help_content,
            help_chrome: &help_chrome,
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

#[cfg(test)]
mod tests {
    #[test]
    fn help_panel_chrome_comes_from_canonical_bindings() {
        let chrome = crate::interface::ui::help_content::build_help_panel_chrome()
            .expect("validated Help chrome");
        assert_eq!(chrome.launcher_shortcut(), "F1");
        assert_eq!(chrome.close_shortcuts(), "F1 / Esc");
        assert_eq!(chrome.topic_navigation().first(), "↑");
        assert_eq!(chrome.topic_navigation().second(), "↓");
        assert_eq!(chrome.page_navigation().first(), "PageUp");
        assert_eq!(chrome.page_navigation().second(), "PageDown");
        assert_eq!(chrome.document_bounds().first(), "Home");
        assert_eq!(chrome.document_bounds().second(), "End");
        for slot in hw_ui::help::HelpChromeSlot::ALL {
            assert!(
                !chrome.shortcut(slot).is_empty(),
                "{} must have a concrete shortcut",
                slot.as_str()
            );
        }
    }
}
