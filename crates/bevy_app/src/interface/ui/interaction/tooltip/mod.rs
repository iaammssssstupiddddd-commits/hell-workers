//! tooltip interaction helpers migrated to hw_ui

pub(crate) use hw_ui::interaction::tooltip::{
    TooltipContentRenderer, TooltipInspectionSource, TooltipRuntimeState, TooltipUiLayoutQueryParam,
};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::popover::Popover;
use hw_ui::components::{
    HoverTooltip, MenuState, PlacementFailureTooltip, TooltipTemplate, UiNodeRegistry,
};
use hw_ui::interaction::tooltip;
use hw_ui::models::inspection::EntityInspectionModel;
use hw_ui::panels::tooltip_builder::TooltipBuildPayload;

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
        payload: TooltipBuildPayload<'_>,
    ) {
        crate::interface::ui::panels::tooltip_builder::rebuild_tooltip_content(
            commands,
            tooltip_root,
            q_children,
            game_assets as &dyn hw_ui::setup::UiAssets,
            theme,
            payload,
        );
    }
}

#[derive(SystemParam)]
pub(crate) struct TooltipStateInput<'w, 's> {
    pub time: Res<'w, Time>,
    pub hovered: Res<'w, crate::interface::selection::HoveredEntity>,
    pub placement_failure_tooltip: ResMut<'w, PlacementFailureTooltip>,
    pub menu_state: Res<'w, MenuState>,
    pub ui_nodes: Res<'w, UiNodeRegistry>,
    pub game_assets: Res<'w, crate::assets::GameAssets>,
    pub theme: Res<'w, hw_ui::theme::UiTheme>,
    pub q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    pub q_tooltip: Query<
        'w,
        's,
        (
            Entity,
            &'static mut HoverTooltip,
            &'static mut Node,
            &'static mut BackgroundColor,
            &'static mut BorderColor,
            &'static mut Popover,
            &'static ComputedNode,
        ),
    >,
}

pub(crate) fn hover_tooltip_system(
    commands: Commands,
    state_input: TooltipStateInput,
    render_queries: tooltip::TooltipRenderQueries,
    ui_layout: TooltipUiLayoutQueryParam,
    inspection: crate::interface::ui::presentation::EntityInspectionQuery<'_, '_>,
    mut runtime: Local<TooltipRuntimeState>,
) {
    let TooltipStateInput {
        time,
        hovered,
        mut placement_failure_tooltip,
        menu_state,
        ui_nodes,
        game_assets,
        theme,
        q_window,
        q_tooltip,
    } = state_input;
    tooltip::hover_tooltip_system(
        commands,
        tooltip::TooltipBevy {
            time: &time,
            hovered: &hovered,
            placement_failure_tooltip: &mut placement_failure_tooltip,
            menu_state: &menu_state,
            ui_nodes: &ui_nodes,
        },
        tooltip::TooltipQuerySet {
            q_window,
            q_tooltip,
            render_queries,
            ui_layout,
        },
        tooltip::TooltipHandlers {
            game_assets: &*game_assets,
            theme: &theme,
            inspection: &inspection,
            tooltip_renderer: &TooltipRenderer,
        },
        &mut runtime,
    );
}
