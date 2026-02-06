use super::{EntityListNodeIndex, EntityListViewModel, FamiliarRowViewModel};
use crate::interface::ui::components::{FamiliarListContainer, UnassignedSoulContent};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

fn sync_familiar_sections(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    view_model: &EntityListViewModel,
    node_index: &mut EntityListNodeIndex,
    fam_container_entity: Entity,
    q_children: &Query<&Children>,
    q_text: &mut Query<&mut Text>,
    q_image: &mut Query<&mut ImageNode>,
) {
    let prev_familiar_ids: HashSet<Entity> = view_model
        .previous
        .familiars
        .iter()
        .map(|vm| vm.entity)
        .collect();
    let curr_familiar_ids: HashSet<Entity> = view_model
        .current
        .familiars
        .iter()
        .map(|vm| vm.entity)
        .collect();

    for familiar_entity in prev_familiar_ids.difference(&curr_familiar_ids) {
        if let Some(nodes) = node_index.familiar_sections.remove(familiar_entity) {
            commands.entity(nodes.root).despawn();
        }
    }

    for familiar in &view_model.current.familiars {
        node_index
            .familiar_sections
            .entry(familiar.entity)
            .or_insert_with(|| {
                super::helpers::spawn_familiar_section(
                    commands,
                    fam_container_entity,
                    familiar,
                    game_assets,
                )
            });
    }

    let previous_by_entity: HashMap<Entity, &FamiliarRowViewModel> = view_model
        .previous
        .familiars
        .iter()
        .map(|vm| (vm.entity, vm))
        .collect();

    for familiar in &view_model.current.familiars {
        let needs_sync = previous_by_entity.get(&familiar.entity) != Some(&familiar);
        if !needs_sync {
            continue;
        }

        if let Some(nodes) = node_index.familiar_sections.get(&familiar.entity).copied() {
            if let Ok(mut text) = q_text.get_mut(nodes.header_text) {
                text.0 = familiar.label.clone();
            }
            if let Ok(mut icon) = q_image.get_mut(nodes.fold_icon) {
                icon.image = if familiar.is_folded {
                    game_assets.icon_arrow_right.clone()
                } else {
                    game_assets.icon_arrow_down.clone()
                };
            }
            super::helpers::sync_familiar_section_content(
                commands, q_children, familiar, nodes, game_assets,
            );
        }
    }
}

fn sync_unassigned_souls(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    view_model: &EntityListViewModel,
    unassigned_content_entity: Entity,
    q_children: &Query<&Children>,
) {
    if view_model.current.unassigned != view_model.previous.unassigned
        || view_model.current.unassigned_folded != view_model.previous.unassigned_folded
    {
        super::helpers::clear_children(commands, q_children, unassigned_content_entity);
        if !view_model.current.unassigned_folded {
            commands
                .entity(unassigned_content_entity)
                .with_children(|parent| {
                    for soul_vm in &view_model.current.unassigned {
                        super::helpers::spawn_soul_list_item(parent, soul_vm, game_assets, 0.0);
                    }
                });
        }
    }
}

pub fn sync_entity_list_from_view_model_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    view_model: Res<EntityListViewModel>,
    mut node_index: ResMut<EntityListNodeIndex>,
    q_fam_container: Query<Entity, With<FamiliarListContainer>>,
    q_unassigned_container: Query<Entity, With<UnassignedSoulContent>>,
    q_children: Query<&Children>,
    mut q_text: Query<&mut Text>,
    mut q_image: Query<&mut ImageNode>,
) {
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
        &view_model,
        unassigned_content_entity,
        &q_children,
    );
}
