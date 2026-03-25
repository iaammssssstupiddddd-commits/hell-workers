use super::{EntityListNodeIndex, EntityListViewModel, SoulGender, StressBucket, TaskVisual};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::{FamiliarListContainer, SoulListItem, UnassignedSoulContent};
use hw_ui::theme::UiTheme;

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

fn dream_color(dream_empty: bool, theme: &UiTheme) -> Color {
    if dream_empty {
        theme.colors.stress_medium
    } else {
        theme.colors.fatigue_text
    }
}

fn gender_icon_and_color(
    gender: SoulGender,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match gender {
        SoulGender::Male => (game_assets.icon_male.clone(), theme.colors.male),
        SoulGender::Female => (game_assets.icon_female.clone(), theme.colors.female),
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

fn lookup_soul_view_model(
    vm: &EntityListViewModel,
    entity: Entity,
) -> Option<&super::SoulRowViewModel> {
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

#[derive(SystemParam)]
pub struct SyncValueMutQueries<'w, 's> {
    q_text: Query<'w, 's, &'static mut Text>,
    q_text_font: Query<'w, 's, &'static mut TextFont>,
    q_text_color: Query<'w, 's, &'static mut TextColor>,
    q_image: Query<'w, 's, &'static mut ImageNode>,
}

pub fn sync_entity_list_value_rows_system(
    resources: SyncValueResources,
    q_soul_rows: Query<(&SoulListItem, &Children)>,
    mutable_queries: SyncValueMutQueries,
    mut dirty: ResMut<super::dirty::EntityListDirty>,
) {
    let SyncValueResources {
        game_assets,
        theme,
        node_index,
        view_model,
    } = resources;
    let SyncValueMutQueries {
        mut q_text,
        mut q_text_font,
        mut q_text_color,
        mut q_image,
    } = mutable_queries;
    for familiar in &view_model.current.familiars {
        let fam_entity = familiar.entity;
        let Some(nodes) = node_index.familiar_sections.get(&fam_entity).copied() else {
            continue;
        };
        if let Ok(mut text) = q_text.get_mut(nodes.header_text) {
            text.0 = familiar.label.clone();
        }
    }

    for (soul_item, children) in q_soul_rows.iter() {
        let Some(soul_vm) = lookup_soul_view_model(&view_model, soul_item.0) else {
            continue;
        };
        if children.len() < 8 {
            continue;
        }
        let gender_node = children[0];
        let name_node = children[1];
        let fatigue_text_node = children[3];
        let stress_text_node = children[5];
        let dream_text_node = children[6];
        let task_icon_node = children[7];
        let stress_color = stress_color(soul_vm.stress_bucket, &theme);
        let dream_color = dream_color(soul_vm.dream_empty, &theme);

        if let Ok(mut text) = q_text.get_mut(name_node) {
            text.0 = soul_vm.name.clone();
        }
        if let Ok(mut color) = q_text_color.get_mut(name_node) {
            color.0 = stress_color;
        }
        if let Ok(mut text) = q_text.get_mut(fatigue_text_node) {
            text.0 = soul_vm.fatigue_text.clone();
        }
        if let Ok(mut text) = q_text.get_mut(stress_text_node) {
            text.0 = soul_vm.stress_text.clone();
        }
        if let Ok(mut color) = q_text_color.get_mut(stress_text_node) {
            color.0 = stress_color;
        }
        if let Ok(mut font) = q_text_font.get_mut(stress_text_node) {
            font.weight = stress_weight(soul_vm.stress_bucket);
        }
        if let Ok(mut text) = q_text.get_mut(dream_text_node) {
            text.0 = soul_vm.dream_text.clone();
        }
        if let Ok(mut color) = q_text_color.get_mut(dream_text_node) {
            color.0 = dream_color;
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
