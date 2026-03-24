// ツールチップ内容の構築

mod templates;
mod text_wrap;
mod widgets;

use crate::components::{TooltipTemplate, UiTooltip};
use crate::list::clear_children;
use crate::models::inspection::EntityInspectionModel;
use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::prelude::*;

pub use text_wrap::{TOOLTIP_WRAP_LIMIT_BODY, TOOLTIP_WRAP_LIMIT_ICON_ROW, wrap_tooltip_text};

#[allow(clippy::too_many_arguments)]
pub fn rebuild_tooltip_content(
    commands: &mut Commands,
    tooltip_root: Entity,
    q_children: &Query<&Children>,
    game_assets: &dyn UiAssets,
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
