use super::super::{EntityListNodeIndex, EntityListViewModel, SoulRowViewModel};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use std::collections::HashMap;

pub(super) fn sync_unassigned_souls(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    view_model: &EntityListViewModel,
    node_index: &mut EntityListNodeIndex,
    unassigned_content_entity: Entity,
    q_children: &Query<&Children>,
) {
    if view_model.current.unassigned_folded != view_model.previous.unassigned_folded {
        super::super::tree_ops::clear_children(commands, q_children, unassigned_content_entity);
        node_index.unassigned_rows.clear();
        if !view_model.current.unassigned_folded {
            for soul_vm in &view_model.current.unassigned {
                let row = super::super::spawn::spawn_soul_list_item_entity(
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
            let row = super::super::spawn::spawn_soul_list_item_entity(
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
            let row = super::super::spawn::spawn_soul_list_item_entity(
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
