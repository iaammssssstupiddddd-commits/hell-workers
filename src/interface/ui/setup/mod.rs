use bevy::prelude::*;
use hw_ui::setup::{UiSetupAssets, setup_ui as hwui_setup_ui};

struct GameAssetsSetupAssets<'a>(&'a crate::assets::GameAssets);

impl<'a> UiSetupAssets for GameAssetsSetupAssets<'a> {
    fn font_ui(&self) -> &Handle<Font> {
        &self.0.font_ui
    }

    fn font_familiar(&self) -> &Handle<Font> {
        &self.0.font_familiar
    }

    fn icon_arrow_down(&self) -> &Handle<Image> {
        &self.0.icon_arrow_down
    }

    fn glow_circle(&self) -> &Handle<Image> {
        &self.0.glow_circle
    }
}

pub fn setup_ui(
    commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<crate::interface::ui::theme::UiTheme>,
    ui_nodes: ResMut<crate::interface::ui::components::UiNodeRegistry>,
    info_panel_nodes: ResMut<crate::interface::ui::components::InfoPanelNodes>,
) {
    let adapter = GameAssetsSetupAssets(&game_assets);
    let theme_ref = &theme;

    hwui_setup_ui(
        commands,
        &adapter,
        theme_ref,
        ui_nodes,
        info_panel_nodes,
        |commands, info_slot, _overlay_slot, ui_nodes, info_panel_nodes| {
            crate::interface::ui::panels::spawn_info_panel_ui(
                commands,
                &game_assets,
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
