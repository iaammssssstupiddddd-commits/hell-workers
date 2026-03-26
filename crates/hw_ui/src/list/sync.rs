// エンティティリスト差分同期 helpers

use super::models::{
    EntityListNodeIndex, EntityListViewModel, FamiliarRowViewModel, FamiliarSectionNodes,
    SoulRowViewModel,
};
use super::spawn::{
    spawn_empty_squad_hint_entity, spawn_familiar_section, spawn_soul_list_item_entity,
};
use super::tree_ops::clear_children;
use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

fn remove_stale_rows<V>(
    commands: &mut Commands,
    row_map: &mut HashMap<Entity, Entity>,
    current_by_entity: &HashMap<Entity, V>,
) {
    let stale: Vec<Entity> = row_map
        .keys()
        .copied()
        .filter(|e| !current_by_entity.contains_key(e))
        .collect();
    for e in stale {
        if let Some(row) = row_map.remove(&e) {
            commands.entity(row).despawn();
        }
    }
}

fn apply_row_order(
    commands: &mut Commands,
    q_children: &Query<&Children>,
    container: Entity,
    ordered_rows: Vec<Entity>,
    clear_if_empty: bool,
) {
    if ordered_rows.is_empty() {
        if clear_if_empty {
            clear_children(commands, q_children, container);
        }
    } else {
        commands.entity(container).replace_children(&ordered_rows);
    }
}

// ============================================================
// Familiar sections
// ============================================================

struct FamiliarRowSyncSpec<'a> {
    familiar: &'a FamiliarRowViewModel,
    previous: Option<&'a FamiliarRowViewModel>,
    nodes: FamiliarSectionNodes,
}

fn sync_familiar_member_rows(
    commands: &mut Commands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    spec: FamiliarRowSyncSpec<'_>,
    node_index: &mut EntityListNodeIndex,
    q_children: &Query<&Children>,
) {
    let FamiliarRowSyncSpec {
        familiar,
        previous,
        nodes,
    } = spec;
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
            clear_children(commands, q_children, nodes.members_container);
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
            let row =
                spawn_empty_squad_hint_entity(commands, nodes.members_container, assets, theme);
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

    remove_stale_rows(commands, member_rows, &current_by_entity);

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
                let row = spawn_soul_list_item_entity(
                    commands,
                    nodes.members_container,
                    soul_vm,
                    assets,
                    theme.sizes.squad_member_left_margin,
                    theme,
                );
                member_rows.insert(soul_vm.entity, row);
            }
        } else {
            let row = spawn_soul_list_item_entity(
                commands,
                nodes.members_container,
                soul_vm,
                assets,
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

    apply_row_order(
        commands,
        q_children,
        nodes.members_container,
        ordered_rows,
        true,
    );
}

pub struct FamiliarSectionCtx<'a> {
    pub view_model: &'a EntityListViewModel,
    pub node_index: &'a mut EntityListNodeIndex,
    pub fam_container_entity: Entity,
}

pub fn sync_familiar_sections(
    commands: &mut Commands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    ctx: &mut FamiliarSectionCtx<'_>,
    q_children: &Query<&Children>,
    q_text: &mut Query<&mut Text>,
    q_image: &mut Query<&mut ImageNode>,
) {
    let view_model = ctx.view_model;
    let node_index = &mut ctx.node_index;
    let fam_container_entity = ctx.fam_container_entity;
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
                spawn_familiar_section(commands, fam_container_entity, familiar, assets, theme)
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
                    assets.icon_arrow_right().clone()
                } else {
                    assets.icon_arrow_down().clone()
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
                    assets,
                    theme,
                    FamiliarRowSyncSpec {
                        familiar,
                        previous,
                        nodes,
                    },
                    node_index,
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
            clear_children(commands, q_children, fam_container_entity);
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

// ============================================================
// Unassigned souls
// ============================================================

pub fn sync_unassigned_souls(
    commands: &mut Commands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    view_model: &EntityListViewModel,
    node_index: &mut EntityListNodeIndex,
    unassigned_content_entity: Entity,
    q_children: &Query<&Children>,
) {
    if view_model.current.unassigned_folded != view_model.previous.unassigned_folded {
        clear_children(commands, q_children, unassigned_content_entity);
        node_index.unassigned_rows.clear();
        if !view_model.current.unassigned_folded {
            for soul_vm in &view_model.current.unassigned {
                let row = spawn_soul_list_item_entity(
                    commands,
                    unassigned_content_entity,
                    soul_vm,
                    assets,
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

    let current_by_entity: HashMap<Entity, &SoulRowViewModel> = view_model
        .current
        .unassigned
        .iter()
        .map(|vm| (vm.entity, vm))
        .collect();
    let previous_by_entity: HashMap<Entity, &SoulRowViewModel> = view_model
        .previous
        .unassigned
        .iter()
        .map(|vm| (vm.entity, vm))
        .collect();

    remove_stale_rows(
        commands,
        &mut node_index.unassigned_rows,
        &current_by_entity,
    );

    for soul_vm in &view_model.current.unassigned {
        if let std::collections::hash_map::Entry::Vacant(e) =
            node_index.unassigned_rows.entry(soul_vm.entity)
        {
            let row = spawn_soul_list_item_entity(
                commands,
                unassigned_content_entity,
                soul_vm,
                assets,
                0.0,
                theme,
            );
            e.insert(row);
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
            let row = spawn_soul_list_item_entity(
                commands,
                unassigned_content_entity,
                soul_vm,
                assets,
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
    apply_row_order(
        commands,
        q_children,
        unassigned_content_entity,
        ordered_rows,
        false,
    );
}
