//! ツールチップ表示・ターゲット解決・フェード制御

mod fade;
mod layout;
mod system;
mod target;

use crate::components::{HoverTooltip, TooltipBody, TooltipHeader, TooltipProgressBar};
use crate::models::inspection::EntityInspectionModel;
use crate::theme::UiTheme;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub use layout::TooltipUiLayoutQueryParam;
pub use system::hover_tooltip_system;
pub use target::TooltipTarget;

type TooltipTextQuery<'w, 's> =
    Query<'w, 's, &'static mut TextColor, Or<(With<TooltipHeader>, With<TooltipBody>)>>;

#[derive(Default)]
pub struct TooltipRuntimeState {
    pub target: Option<TooltipTarget>,
    pub payload: String,
    pub attach_to_anchor: bool,
}

pub trait TooltipInspectionSource {
    fn build_model(&self, entity: Entity) -> Option<EntityInspectionModel>;
    fn classify_template(&self, entity: Entity) -> crate::components::TooltipTemplate;
}

pub trait TooltipContentRenderer {
    type GameAssets;

    #[allow(clippy::too_many_arguments)]
    fn rebuild_tooltip_content(
        &self,
        commands: &mut Commands,
        tooltip_root: Entity,
        q_children: &Query<&Children>,
        game_assets: &Self::GameAssets,
        theme: &UiTheme,
        template: crate::components::TooltipTemplate,
        model: Option<&EntityInspectionModel>,
        ui_tooltip: Option<&crate::components::UiTooltip>,
    );
}

#[derive(SystemParam)]
pub struct TooltipRenderQueries<'w, 's> {
    pub q_children: Query<'w, 's, &'static Children>,
    pub q_nodes: Query<'w, 's, &'static mut Node, Without<HoverTooltip>>,
    pub q_tooltip_text: TooltipTextQuery<'w, 's>,
    pub q_tooltip_progress: Query<
        'w,
        's,
        (&'static TooltipProgressBar, &'static mut BackgroundColor),
        Without<HoverTooltip>,
    >,
}
