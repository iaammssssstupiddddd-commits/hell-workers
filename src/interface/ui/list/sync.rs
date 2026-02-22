use super::{EntityListNodeIndex, EntityListViewModel, StressBucket, TaskVisual};
use crate::entities::damned_soul::{DamnedSoul, Gender, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::ui::components::{
    FamiliarListContainer, SoulListItem, UnassignedSoulContent,
};
use crate::interface::ui::theme::UiTheme;
use crate::relationships::Commanding;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;

mod familiar;
mod unassigned;

use familiar::sync_familiar_sections;
use unassigned::sync_unassigned_souls;

fn stress_color(bucket: StressBucket, theme: &UiTheme) -> Color {
    match bucket {
        StressBucket::Low => Color::WHITE,
        StressBucket::Medium => theme.colors.stress_medium,
        StressBucket::High => theme.colors.stress_high,
    }
}

fn stress_weight(bucket: StressBucket) -> FontWeight {
    match bucket {
        StressBucket::High => FontWeight::BOLD,
        _ => FontWeight::default(),
    }
}

fn gender_icon_and_color(
    gender: Gender,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match gender {
        Gender::Male => (game_assets.icon_male.clone(), theme.colors.male),
        Gender::Female => (game_assets.icon_female.clone(), theme.colors.female),
    }
}

fn task_icon_and_color(
    task_visual: TaskVisual,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match task_visual {
        TaskVisual::Idle => (game_assets.icon_idle.clone(), theme.colors.idle),
        TaskVisual::Chop => (game_assets.icon_axe.clone(), theme.colors.chop),
        TaskVisual::Mine => (game_assets.icon_pick.clone(), theme.colors.mine),
        TaskVisual::GatherDefault => (game_assets.icon_pick.clone(), theme.colors.gather_default),
        TaskVisual::Haul => (game_assets.icon_haul.clone(), theme.colors.haul),
        TaskVisual::Build => (game_assets.icon_pick.clone(), theme.colors.build),
        TaskVisual::HaulToBlueprint => (game_assets.icon_haul.clone(), theme.colors.haul_to_bp),
        TaskVisual::Water => (game_assets.icon_haul.clone(), theme.colors.water),
    }
}

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
    dirty.clear_all();

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

pub fn sync_entity_list_value_rows_system(
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
    node_index: Res<EntityListNodeIndex>,
    q_familiars: Query<(
        Entity,
        &Familiar,
        &FamiliarOperation,
        &FamiliarAiState,
        Option<&Commanding>,
    )>,
    q_souls: Query<(Entity, &DamnedSoul, &AssignedTask, &SoulIdentity)>,
    q_soul_rows: Query<(&SoulListItem, &Children)>,
    mut q_text: Query<&mut Text>,
    mut q_text_font: Query<&mut TextFont>,
    mut q_text_color: Query<&mut TextColor>,
    mut q_image: Query<&mut ImageNode>,
    mut dirty: ResMut<super::dirty::EntityListDirty>,
) {
    for (fam_entity, familiar, op, ai_state, commanding_opt) in q_familiars.iter() {
        let Some(nodes) = node_index.familiar_sections.get(&fam_entity).copied() else {
            continue;
        };
        if let Ok(mut text) = q_text.get_mut(nodes.header_text) {
            let squad_count = commanding_opt.map(|c| c.len()).unwrap_or(0);
            text.0 = super::view_model::familiar_label(familiar, op, ai_state, squad_count);
        }
    }

    for (soul_item, children) in q_soul_rows.iter() {
        let Ok((soul_entity, soul, task, identity)) = q_souls.get(soul_item.0) else {
            continue;
        };
        if children.len() < 7 {
            continue;
        }

        let soul_vm = super::view_model::build_soul_view_model(soul_entity, soul, task, identity);
        let gender_node = children[0];
        let name_node = children[1];
        let fatigue_text_node = children[3];
        let stress_text_node = children[5];
        let task_icon_node = children[6];
        let stress_color = stress_color(soul_vm.stress_bucket, &theme);

        if let Ok(mut text) = q_text.get_mut(name_node) {
            text.0 = soul_vm.name;
        }
        if let Ok(mut color) = q_text_color.get_mut(name_node) {
            color.0 = stress_color;
        }
        if let Ok(mut text) = q_text.get_mut(fatigue_text_node) {
            text.0 = soul_vm.fatigue_text;
        }
        if let Ok(mut text) = q_text.get_mut(stress_text_node) {
            text.0 = soul_vm.stress_text;
        }
        if let Ok(mut color) = q_text_color.get_mut(stress_text_node) {
            color.0 = stress_color;
        }
        if let Ok(mut font) = q_text_font.get_mut(stress_text_node) {
            font.weight = stress_weight(soul_vm.stress_bucket);
        }

        let (gender_icon, gender_color) =
            gender_icon_and_color(soul_vm.gender, &game_assets, &theme);
        if let Ok(mut image) = q_image.get_mut(gender_node) {
            image.image = gender_icon;
            image.color = gender_color;
        }
        let (task_icon, task_color) =
            task_icon_and_color(soul_vm.task_visual, &game_assets, &theme);
        if let Ok(mut image) = q_image.get_mut(task_icon_node) {
            image.image = task_icon;
            image.color = task_color;
        }
    }

    dirty.clear_values();
}
