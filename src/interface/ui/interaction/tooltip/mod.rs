//! tooltip interaction helpers migrated to hw_ui

pub(crate) use hw_ui::interaction::tooltip::{
    TooltipContentRenderer, TooltipInspectionSource, TooltipRuntimeState, TooltipUiLayoutQueryParam,
};

use bevy::prelude::*;
use bevy::ui_widgets::popover::Popover;
use hw_ui::components::{
    HoverTooltip, MenuState, PlacementFailureTooltip, TooltipTemplate, UiNodeRegistry, UiTooltip,
};
use hw_ui::interaction::tooltip;
use hw_ui::models::inspection::EntityInspectionModel;

impl TooltipInspectionSource for crate::interface::ui::presentation::EntityInspectionQuery<'_, '_> {
    fn build_model(&self, entity: Entity) -> Option<EntityInspectionModel> {
        self.build_model(entity)
    }

    fn classify_template(&self, entity: Entity) -> TooltipTemplate {
        self.classify_template(entity)
    }
}

struct TooltipRenderer;

impl TooltipContentRenderer for TooltipRenderer {
    type GameAssets = crate::assets::GameAssets;

    fn rebuild_tooltip_content(
        &self,
        commands: &mut Commands,
        tooltip_root: Entity,
        q_children: &Query<&Children>,
        game_assets: &Self::GameAssets,
        theme: &hw_ui::theme::UiTheme,
        template: TooltipTemplate,
        model: Option<&EntityInspectionModel>,
        ui_tooltip: Option<&UiTooltip>,
    ) {
        crate::interface::ui::panels::tooltip_builder::rebuild_tooltip_content(
            commands,
            tooltip_root,
            q_children,
            game_assets,
            theme,
            template,
            model,
            ui_tooltip,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn hover_tooltip_system(
    commands: Commands,
    time: Res<Time>,
    hovered: Res<crate::interface::selection::HoveredEntity>,
    placement_failure_tooltip: ResMut<PlacementFailureTooltip>,
    menu_state: Res<MenuState>,
    ui_nodes: Res<UiNodeRegistry>,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<hw_ui::theme::UiTheme>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_tooltip: Query<(
        Entity,
        &mut HoverTooltip,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Popover,
        &ComputedNode,
    )>,
    render_queries: tooltip::TooltipRenderQueries,
    ui_layout: TooltipUiLayoutQueryParam,
    inspection: crate::interface::ui::presentation::EntityInspectionQuery<'_, '_>,
    mut runtime: Local<TooltipRuntimeState>,
) {
    tooltip::hover_tooltip_system(
        commands,
        time,
        hovered,
        placement_failure_tooltip,
        menu_state,
        ui_nodes,
        &*game_assets,
        &theme,
        q_window,
        q_tooltip,
        render_queries,
        ui_layout,
        &inspection,
        &mut runtime,
        &TooltipRenderer,
    );
}
