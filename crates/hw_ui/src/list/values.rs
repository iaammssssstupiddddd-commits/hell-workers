use super::models::{EntityListNodeIndex, EntityListViewModel, SoulRowNodes, SoulRowViewModel};
use super::style::*;
use crate::{components::SoulListItem, setup::UiAssets, theme::UiTheme};
use bevy::{ecs::system::SystemParam, prelude::*};

#[cfg(test)]
mod tests;

fn lookup_soul_view_model(vm: &EntityListViewModel, entity: Entity) -> Option<&SoulRowViewModel> {
    for familiar in &vm.current.familiars {
        if let Some(found) = familiar.souls.iter().find(|soul| soul.entity == entity) {
            return Some(found);
        }
    }
    vm.current
        .unassigned
        .iter()
        .find(|soul| soul.entity == entity)
}

#[derive(SystemParam)]
pub struct EntityListValueQueries<'w, 's> {
    q_soul_rows: Query<'w, 's, (&'static SoulListItem, &'static SoulRowNodes)>,
    q_text: Query<'w, 's, &'static mut Text>,
    q_text_font: Query<'w, 's, &'static mut TextFont>,
    q_text_color: Query<'w, 's, &'static mut TextColor>,
    q_image: Query<'w, 's, &'static mut ImageNode>,
}

pub fn sync_entity_list_values(
    assets: &dyn UiAssets,
    theme: &UiTheme,
    view_model: &EntityListViewModel,
    node_index: &EntityListNodeIndex,
    queries: &mut EntityListValueQueries,
) {
    let EntityListValueQueries {
        q_soul_rows,
        q_text,
        q_text_font,
        q_text_color,
        q_image,
    } = queries;
    for familiar in &view_model.current.familiars {
        let fam_entity = familiar.entity;
        let Some(nodes) = node_index.familiar_sections.get(&fam_entity).copied() else {
            continue;
        };
        if let Ok(mut text) = q_text.get_mut(nodes.header_text)
            && text.0 != familiar.label
        {
            text.0 = familiar.label.clone();
        }
    }

    for (soul_item, nodes) in q_soul_rows.iter() {
        let Some(soul_vm) = lookup_soul_view_model(view_model, soul_item.0) else {
            continue;
        };
        let gender_node = nodes.gender_icon;
        let name_node = nodes.name_text;
        let fatigue_text_node = nodes.fatigue_text;
        let stress_text_node = nodes.stress_text;
        let dream_text_node = nodes.dream_text;
        let task_icon_node = nodes.task_icon;
        let stress_color = get_stress_color(soul_vm.stress_bucket, theme);
        let dream_color = get_dream_color(soul_vm.dream_empty, theme);

        if let Ok(mut text) = q_text.get_mut(name_node)
            && text.0 != soul_vm.name
        {
            text.0 = soul_vm.name.clone();
        }
        if let Ok(mut color) = q_text_color.get_mut(name_node)
            && color.0 != stress_color
        {
            color.0 = stress_color;
        }
        if let Ok(mut text) = q_text.get_mut(fatigue_text_node)
            && text.0 != soul_vm.fatigue_text
        {
            text.0 = soul_vm.fatigue_text.clone();
        }
        if let Ok(mut text) = q_text.get_mut(stress_text_node)
            && text.0 != soul_vm.stress_text
        {
            text.0 = soul_vm.stress_text.clone();
        }
        if let Ok(mut color) = q_text_color.get_mut(stress_text_node)
            && color.0 != stress_color
        {
            color.0 = stress_color;
        }
        let stress_weight = stress_weight(soul_vm.stress_bucket);
        if let Ok(mut font) = q_text_font.get_mut(stress_text_node)
            && font.weight != stress_weight
        {
            font.weight = stress_weight;
        }
        if let Ok(mut text) = q_text.get_mut(dream_text_node)
            && text.0 != soul_vm.dream_text
        {
            text.0 = soul_vm.dream_text.clone();
        }
        if let Ok(mut color) = q_text_color.get_mut(dream_text_node)
            && color.0 != dream_color
        {
            color.0 = dream_color;
        }

        let (gender_icon, gender_color) = get_gender_icon_and_color(soul_vm.gender, assets, theme);
        if let Ok(mut image) = q_image.get_mut(gender_node)
            && (image.image != gender_icon || image.color != gender_color)
        {
            image.image = gender_icon;
            image.color = gender_color;
        }
        let (task_icon, task_color) = get_task_icon_and_color(soul_vm.task_visual, assets, theme);
        {
            let label_node = nodes.task_label;
            if let Ok(mut text) = q_text.get_mut(label_node)
                && text.0 != soul_vm.task_visual.label()
            {
                text.0 = soul_vm.task_visual.label().into();
            }
            if let Ok(mut color) = q_text_color.get_mut(label_node)
                && color.0 != task_color
            {
                color.0 = task_color;
            }
        }
        if let Ok(mut image) = q_image.get_mut(task_icon_node)
            && (image.image != task_icon || image.color != task_color)
        {
            image.image = task_icon;
            image.color = task_color;
        }
    }
}
