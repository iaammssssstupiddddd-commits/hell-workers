//! ツールチップ内容の構築

mod templates;
mod text_wrap;
mod widgets;

use crate::assets::GameAssets;
use crate::interface::ui::components::{TooltipTemplate, UiTooltip};
use crate::interface::ui::list::clear_children;
use crate::interface::ui::presentation::EntityInspectionModel;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

pub fn rebuild_tooltip_content(
    commands: &mut Commands,
    tooltip_root: Entity,
    q_children: &Query<&Children>,
    game_assets: &GameAssets,
    theme: &UiTheme,
    template: TooltipTemplate,
    model: Option<&EntityInspectionModel>,
    ui_tooltip: Option<&UiTooltip>,
) {
    clear_children(commands, q_children, tooltip_root);

    commands
        .entity(tooltip_root)
        .with_children(|parent| match template {
            TooltipTemplate::Soul => {
                templates::build_soul_tooltip(parent, model, game_assets, theme)
            }
            TooltipTemplate::Building => {
                templates::build_building_tooltip(parent, model, game_assets, theme)
            }
            TooltipTemplate::Resource => {
                templates::build_resource_tooltip(parent, model, game_assets, theme)
            }
            TooltipTemplate::UiButton => {
                templates::build_ui_button_tooltip(parent, ui_tooltip, game_assets, theme)
            }
            TooltipTemplate::Generic => {
                templates::build_generic_tooltip(parent, model, ui_tooltip, game_assets, theme)
            }
        });
}
