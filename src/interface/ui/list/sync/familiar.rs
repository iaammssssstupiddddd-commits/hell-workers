use super::super::{
    EntityListNodeIndex, EntityListViewModel, FamiliarRowViewModel, FamiliarSectionNodes,
    SoulRowViewModel,
};
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
    nodes: FamiliarSectionNodes,
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
            super::super::tree_ops::clear_children(commands, q_children, nodes.members_container);
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
            let row = super::super::spawn::spawn_empty_squad_hint_entity(
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
                let row = super::super::spawn::spawn_soul_list_item_entity(
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
            let row = super::super::spawn::spawn_soul_list_item_entity(
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
        super::super::tree_ops::clear_children(commands, q_children, nodes.members_container);
    } else {
        commands
            .entity(nodes.members_container)
            .replace_children(&ordered_rows);
    }
}

pub(super) fn sync_familiar_sections(
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
                super::super::spawn::spawn_familiar_section(
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
            super::super::tree_ops::clear_children(commands, q_children, fam_container_entity);
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
