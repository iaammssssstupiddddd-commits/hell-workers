use super::{EntityListNodeIndex, EntityListViewModel, FamiliarRowViewModel, SoulRowViewModel};
use crate::interface::ui::components::{FamiliarListContainer, UnassignedSoulContent};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

fn sync_familiar_member_rows(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    familiar: &FamiliarRowViewModel,
    previous: Option<&FamiliarRowViewModel>,
    node_index: &mut EntityListNodeIndex,
    nodes: super::FamiliarSectionNodes,
    q_children: &Query<&Children>,
) {
    let member_rows = node_index
        .familiar_member_rows
        .entry(familiar.entity)
        .or_default();

    if familiar.is_folded {
        if !member_rows.is_empty()
            || node_index
                .familiar_empty_rows
                .contains_key(&familiar.entity)
        {
            super::tree_ops::clear_children(commands, q_children, nodes.members_container);
            member_rows.clear();
            node_index.familiar_empty_rows.remove(&familiar.entity);
        }
        return;
    }

    if familiar.show_empty {
        let existing_rows: Vec<Entity> = member_rows.drain().map(|(_, row)| row).collect();
        for row in existing_rows {
            commands.entity(row).despawn();
        }

        let empty_row = if let Some(row) = node_index.familiar_empty_rows.get(&familiar.entity) {
            *row
        } else {
            let row = super::spawn::spawn_empty_squad_hint_entity(
                commands,
                nodes.members_container,
                game_assets,
                theme,
            );
            node_index.familiar_empty_rows.insert(familiar.entity, row);
            row
        };

        commands
            .entity(nodes.members_container)
            .replace_children(&[empty_row]);
        return;
    }

    if let Some(empty_row) = node_index.familiar_empty_rows.remove(&familiar.entity) {
        commands.entity(empty_row).despawn();
    }

    let current_by_entity: HashMap<Entity, &SoulRowViewModel> =
        familiar.souls.iter().map(|vm| (vm.entity, vm)).collect();

    let stale_entities: Vec<Entity> = member_rows
        .keys()
        .copied()
        .filter(|entity| !current_by_entity.contains_key(entity))
        .collect();
    for entity in stale_entities {
        if let Some(row) = member_rows.remove(&entity) {
            commands.entity(row).despawn();
        }
    }

    let previous_souls: HashMap<Entity, &SoulRowViewModel> = previous
        .map(|vm| vm.souls.iter().map(|soul| (soul.entity, soul)).collect())
        .unwrap_or_default();

    for soul_vm in &familiar.souls {
        let has_changed = previous_souls
            .get(&soul_vm.entity)
            .map(|prev| *prev != soul_vm)
            .unwrap_or(true);

        if let Some(existing_row) = member_rows.get(&soul_vm.entity).copied() {
            if has_changed {
                commands.entity(existing_row).despawn();
                let row = super::spawn::spawn_soul_list_item_entity(
                    commands,
                    nodes.members_container,
                    soul_vm,
                    game_assets,
                    theme.sizes.squad_member_left_margin,
                    theme,
                );
                member_rows.insert(soul_vm.entity, row);
            }
        } else {
            let row = super::spawn::spawn_soul_list_item_entity(
                commands,
                nodes.members_container,
                soul_vm,
                game_assets,
                theme.sizes.squad_member_left_margin,
                theme,
            );
            member_rows.insert(soul_vm.entity, row);
        }
    }

    let ordered_rows: Vec<Entity> = familiar
        .souls
        .iter()
        .filter_map(|vm| member_rows.get(&vm.entity).copied())
        .collect();

    if ordered_rows.is_empty() {
        super::tree_ops::clear_children(commands, q_children, nodes.members_container);
    } else {
        commands
            .entity(nodes.members_container)
            .replace_children(&ordered_rows);
    }
}

fn sync_familiar_sections(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
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
        node_index.familiar_member_rows.remove(familiar_entity);
        node_index.familiar_empty_rows.remove(familiar_entity);
    }

    for familiar in &view_model.current.familiars {
        node_index
            .familiar_sections
            .entry(familiar.entity)
            .or_insert_with(|| {
                super::spawn::spawn_familiar_section(
                    commands,
                    fam_container_entity,
                    familiar,
                    game_assets,
                    theme,
                )
            });
        node_index
            .familiar_member_rows
            .entry(familiar.entity)
            .or_default();
    }

    let previous_by_entity: HashMap<Entity, &FamiliarRowViewModel> = view_model
        .previous
        .familiars
        .iter()
        .map(|vm| (vm.entity, vm))
        .collect();

    for familiar in &view_model.current.familiars {
        let previous = previous_by_entity.get(&familiar.entity).copied();
        let needs_sync = previous != Some(familiar);
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
            let members_changed = previous
                .map(|prev| {
                    prev.is_folded != familiar.is_folded
                        || prev.show_empty != familiar.show_empty
                        || prev.souls != familiar.souls
                })
                .unwrap_or(true);
            if members_changed {
                sync_familiar_member_rows(
                    commands,
                    game_assets,
                    theme,
                    familiar,
                    previous,
                    node_index,
                    nodes,
                    q_children,
                );
            }
        }
    }

    let ordered_sections: Vec<Entity> = view_model
        .current
        .familiars
        .iter()
        .filter_map(|familiar| {
            node_index
                .familiar_sections
                .get(&familiar.entity)
                .map(|nodes| nodes.root)
        })
        .collect();
    if ordered_sections.is_empty() {
        if q_children
            .get(fam_container_entity)
            .map(|children| !children.is_empty())
            .unwrap_or(false)
        {
            super::tree_ops::clear_children(commands, q_children, fam_container_entity);
        }
    } else {
        let needs_reorder = q_children
            .get(fam_container_entity)
            .map(|children| {
                children.len() != ordered_sections.len()
                    || !children
                        .iter()
                        .zip(ordered_sections.iter())
                        .all(|(a, b)| a == *b)
            })
            .unwrap_or(true);
        if needs_reorder {
            commands
                .entity(fam_container_entity)
                .replace_children(&ordered_sections);
        }
    }
}

fn sync_unassigned_souls(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    view_model: &EntityListViewModel,
    node_index: &mut EntityListNodeIndex,
    unassigned_content_entity: Entity,
    q_children: &Query<&Children>,
) {
    if view_model.current.unassigned_folded != view_model.previous.unassigned_folded {
        super::tree_ops::clear_children(commands, q_children, unassigned_content_entity);
        node_index.unassigned_rows.clear();
        if !view_model.current.unassigned_folded {
            for soul_vm in &view_model.current.unassigned {
                let row = super::spawn::spawn_soul_list_item_entity(
                    commands,
                    unassigned_content_entity,
                    soul_vm,
                    game_assets,
                    0.0,
                    theme,
                );
                node_index.unassigned_rows.insert(soul_vm.entity, row);
            }
        }
        return;
    }

    if view_model.current.unassigned_folded {
        return;
    }

    if view_model.current.unassigned == view_model.previous.unassigned {
        return;
    }

    let current_by_entity: HashMap<Entity, &super::SoulRowViewModel> = view_model
        .current
        .unassigned
        .iter()
        .map(|vm| (vm.entity, vm))
        .collect();
    let previous_by_entity: HashMap<Entity, &super::SoulRowViewModel> = view_model
        .previous
        .unassigned
        .iter()
        .map(|vm| (vm.entity, vm))
        .collect();

    let stale_entities: Vec<Entity> = node_index
        .unassigned_rows
        .keys()
        .copied()
        .filter(|entity| !current_by_entity.contains_key(entity))
        .collect();
    for entity in stale_entities {
        if let Some(row) = node_index.unassigned_rows.remove(&entity) {
            commands.entity(row).despawn();
        }
    }

    for soul_vm in &view_model.current.unassigned {
        if !node_index.unassigned_rows.contains_key(&soul_vm.entity) {
            let row = super::spawn::spawn_soul_list_item_entity(
                commands,
                unassigned_content_entity,
                soul_vm,
                game_assets,
                0.0,
                theme,
            );
            node_index.unassigned_rows.insert(soul_vm.entity, row);
            continue;
        }

        let has_changed = previous_by_entity
            .get(&soul_vm.entity)
            .map(|prev| *prev != soul_vm)
            .unwrap_or(true);
        if has_changed {
            if let Some(old_row) = node_index.unassigned_rows.remove(&soul_vm.entity) {
                commands.entity(old_row).despawn();
            }
            let row = super::spawn::spawn_soul_list_item_entity(
                commands,
                unassigned_content_entity,
                soul_vm,
                game_assets,
                0.0,
                theme,
            );
            node_index.unassigned_rows.insert(soul_vm.entity, row);
        }
    }

    let ordered_rows: Vec<Entity> = view_model
        .current
        .unassigned
        .iter()
        .filter_map(|vm| node_index.unassigned_rows.get(&vm.entity).copied())
        .collect();
    if !ordered_rows.is_empty() {
        commands
            .entity(unassigned_content_entity)
            .replace_children(&ordered_rows);
    }
}

pub fn sync_entity_list_from_view_model_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
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
