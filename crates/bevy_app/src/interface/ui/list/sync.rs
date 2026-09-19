use super::{EntityListNodeIndex, EntityListViewModel};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::{FamiliarListContainer, UnassignedSoulContent};
use hw_ui::theme::UiTheme;

#[derive(SystemParam)]
pub struct SyncViewModelCtx<'w, 's> {
    game_assets: Res<'w, crate::assets::GameAssets>,
    theme: Res<'w, UiTheme>,
    view_model: ResMut<'w, EntityListViewModel>,
    node_index: ResMut<'w, EntityListNodeIndex>,
    q_fam_container: Query<'w, 's, Entity, With<FamiliarListContainer>>,
    q_unassigned_container: Query<'w, 's, Entity, With<UnassignedSoulContent>>,
    q_children: Query<'w, 's, &'static Children>,
}

#[derive(SystemParam)]
pub struct SyncMutUiQueries<'w, 's> {
    q_text: Query<'w, 's, &'static mut Text>,
    q_image: Query<'w, 's, &'static mut ImageNode>,
}

pub fn sync_entity_list_from_view_model_system(
    mut commands: Commands,
    mut ctx: SyncViewModelCtx,
    mut dirty: ResMut<super::dirty::EntityListDirty>,
    mut ui_queries: SyncMutUiQueries,
) {
    dirty.clear_all();

    if ctx.view_model.current == ctx.view_model.previous {
        return;
    }

    let fam_container_entity = if let Some(e) = ctx.q_fam_container.iter().next() {
        e
    } else {
        return;
    };
    let unassigned_content_entity = if let Some(e) = ctx.q_unassigned_container.iter().next() {
        e
    } else {
        return;
    };

    hw_ui::list::sync::sync_familiar_sections(
        &mut commands,
        ctx.game_assets.as_ref() as &dyn hw_ui::setup::UiAssets,
        &ctx.theme,
        &mut hw_ui::list::FamiliarSectionCtx {
            view_model: &ctx.view_model,
            node_index: &mut ctx.node_index,
            fam_container_entity,
        },
        &ctx.q_children,
        &mut ui_queries.q_text,
        &mut ui_queries.q_image,
    );
    hw_ui::list::sync::sync_unassigned_souls(
        &mut commands,
        ctx.game_assets.as_ref() as &dyn hw_ui::setup::UiAssets,
        &ctx.theme,
        &ctx.view_model,
        &mut ctx.node_index,
        unassigned_content_entity,
        &ctx.q_children,
    );

    ctx.view_model.previous = ctx.view_model.current.clone();
}

#[derive(SystemParam)]
pub struct SyncValueResources<'w> {
    game_assets: Res<'w, crate::assets::GameAssets>,
    theme: Res<'w, UiTheme>,
    node_index: Res<'w, EntityListNodeIndex>,
    view_model: Res<'w, EntityListViewModel>,
}

pub fn sync_entity_list_value_rows_system(
    resources: SyncValueResources,
    mut queries: hw_ui::list::values::EntityListValueQueries,
    mut dirty: ResMut<super::dirty::EntityListDirty>,
) {
    hw_ui::list::values::sync_entity_list_values(
        resources.game_assets.as_ref(),
        &resources.theme,
        &resources.view_model,
        &resources.node_index,
        &mut queries,
    );
    dirty.clear_values();
}
