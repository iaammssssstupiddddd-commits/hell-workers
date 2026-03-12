use bevy::prelude::*;
use hw_ui::setup::setup_ui as hwui_setup_ui;

pub fn setup_ui(
    commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<hw_ui::theme::UiTheme>,
    ui_nodes: ResMut<hw_ui::components::UiNodeRegistry>,
    info_panel_nodes: ResMut<hw_ui::components::InfoPanelNodes>,
) {
    let theme_ref = &theme;

    hwui_setup_ui(
        commands,
        &*game_assets,
        theme_ref,
        ui_nodes,
        info_panel_nodes,
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
