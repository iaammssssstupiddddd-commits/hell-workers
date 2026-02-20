use super::{EntityListNodeIndex, EntityListViewModel};
use crate::interface::ui::components::{FamiliarListContainer, UnassignedSoulContent};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

mod familiar;
mod unassigned;

use familiar::sync_familiar_sections;
use unassigned::sync_unassigned_souls;

pub fn sync_entity_list_from_view_model_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
    view_model: Res<EntityListViewModel>,
    mut node_index: ResMut<EntityListNodeIndex>,
    mut dirty: ResMut<super::dirty::EntityListDirty>,
    q_fam_container: Query<Entity, With<FamiliarListContainer>>,
    q_unassigned_container: Query<Entity, With<UnassignedSoulContent>>,
    q_children: Query<&Children>,
    mut q_text: Query<&mut Text>,
    mut q_image: Query<&mut ImageNode>,
) {
    dirty.clear();

    if view_model.current == view_model.previous {
        return;
    }

    let fam_container_entity = if let Some(e) = q_fam_container.iter().next() {
        e
    } else {
        return;
    };
    let unassigned_content_entity = if let Some(e) = q_unassigned_container.iter().next() {
        e
    } else {
        return;
    };

    sync_familiar_sections(
        &mut commands,
        &game_assets,
        &theme,
        &view_model,
        &mut node_index,
        fam_container_entity,
        &q_children,
        &mut q_text,
        &mut q_image,
    );
    sync_unassigned_souls(
        &mut commands,
        &game_assets,
        &theme,
        &view_model,
        &mut node_index,
        unassigned_content_entity,
        &q_children,
    );
}
