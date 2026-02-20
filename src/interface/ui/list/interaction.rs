use super::EntityListNodeIndex;
use crate::entities::familiar::FamiliarOperation;
use crate::events::FamiliarOperationMaxSoulChangedEvent;
use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use crate::relationships::Commanding;
use crate::systems::familiar_ai::FamiliarAiState;
use bevy::prelude::*;

mod navigation;
mod visual;

pub use navigation::{
    entity_list_scroll_hint_visibility_system, entity_list_scroll_system,
    entity_list_tab_focus_system, update_unassigned_arrow_icon_system,
};
pub use visual::{apply_row_highlight, entity_list_visual_feedback_system};

fn toggle_list_section(
    commands: &mut Commands,
    section_type: EntityListSectionType,
    q_folded: &Query<Has<SectionFolded>>,
    unassigned_folded_query: &Query<(Entity, Has<UnassignedFolded>), With<UnassignedSoulSection>>,
) {
    match section_type {
        EntityListSectionType::Familiar(entity) => {
            if q_folded.get(entity).unwrap_or(false) {
                commands.entity(entity).remove::<SectionFolded>();
            } else {
                commands.entity(entity).insert(SectionFolded);
            }
        }
        EntityListSectionType::Unassigned => {
            let mut any_toggled = false;
            for (unassigned_entity, has_folded) in unassigned_folded_query.iter() {
                if has_folded {
                    commands.entity(unassigned_entity).remove::<UnassignedFolded>();
                } else {
                    commands.entity(unassigned_entity).insert(UnassignedFolded);
                }
                any_toggled = true;
            }
            if !any_toggled {
                warn!("LIST: UnassignedSoulSection not found for toggling!");
            }
        }
    }
}

fn focus_list_entity(
    entity: Entity,
    label: &'static str,
    selected_entity: &mut ResMut<crate::interface::selection::SelectedEntity>,
    q_camera: &mut Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: &Query<&GlobalTransform>,
) {
    super::selection_focus::select_entity_and_focus_camera(
        entity,
        label,
        selected_entity,
        q_camera,
        q_transforms,
    );
}

fn handle_familiar_max_soul_adjustment(
    button: FamiliarMaxSoulAdjustButton,
    q_familiar_ops: &mut Query<&mut FamiliarOperation>,
    q_familiar_meta: &Query<(
        &crate::entities::familiar::Familiar,
        &FamiliarAiState,
        Option<&Commanding>,
    )>,
    node_index: &EntityListNodeIndex,
    q_text: &mut Query<&mut Text>,
    ev_max_soul_changed: &mut MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
) {
    if let Ok(mut op) = q_familiar_ops.get_mut(button.familiar) {
        let old_val = op.max_controlled_soul;
        let new_val = (old_val as isize + button.delta).clamp(1, 8) as usize;
        op.max_controlled_soul = new_val;

        if let Some(nodes) = node_index.familiar_sections.get(&button.familiar)
            && let Ok((familiar, ai_state, commanding_opt)) = q_familiar_meta.get(button.familiar)
            && let Ok(mut text) = q_text.get_mut(nodes.header_text)
        {
            let squad_count = commanding_opt.map(|c| c.len()).unwrap_or(0);
            text.0 = format!(
                "{} ({}/{}) [{}]",
                familiar.name,
                squad_count,
                new_val,
                super::view_model::familiar_state_label(ai_state)
            );
        }

        if old_val != new_val {
            ev_max_soul_changed.write(FamiliarOperationMaxSoulChangedEvent {
                familiar_entity: button.familiar,
                old_value: old_val,
                new_value: new_val,
            });
        }
    }
}

/// エンティティリストのインタラクション
pub fn entity_list_interaction_system(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SectionToggle, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut soul_list_interaction: Query<
        (&Interaction, &SoulListItem),
        (
            Changed<Interaction>,
            With<Button>,
            Without<FamiliarListItem>,
        ),
    >,
    mut familiar_list_interaction: Query<
        (&Interaction, &FamiliarListItem),
        (Changed<Interaction>, With<Button>, Without<SoulListItem>),
    >,
    mut familiar_max_soul_buttons: Query<
        (
            &Interaction,
            &FamiliarMaxSoulAdjustButton,
            &mut BackgroundColor,
        ),
        (
            Changed<Interaction>,
            With<Button>,
            Without<FamiliarListItem>,
            Without<SoulListItem>,
            Without<SectionToggle>,
        ),
    >,
    mut q_familiar_ops: Query<&mut FamiliarOperation>,
    q_familiar_meta: Query<(
        &crate::entities::familiar::Familiar,
        &FamiliarAiState,
        Option<&Commanding>,
    )>,
    node_index: Res<EntityListNodeIndex>,
    mut q_text: Query<&mut Text>,
    mut ev_max_soul_changed: MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    mut q_camera: Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: Query<&GlobalTransform>,
    q_folded: Query<Has<SectionFolded>>,
    unassigned_folded_query: Query<(Entity, Has<UnassignedFolded>), With<UnassignedSoulSection>>,
    theme: Res<UiTheme>,
) {
    for (interaction, toggle, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(theme.colors.section_toggle_pressed);
                toggle_list_section(&mut commands, toggle.0, &q_folded, &unassigned_folded_query);
            }
            Interaction::Hovered => {
                *color = BackgroundColor(theme.colors.button_hover);
            }
            Interaction::None => {
                *color = BackgroundColor(theme.colors.button_default);
            }
        }
    }

    for (interaction, item) in soul_list_interaction.iter_mut() {
        if *interaction == Interaction::Pressed {
            focus_list_entity(
                item.0,
                "soul",
                &mut selected_entity,
                &mut q_camera,
                &q_transforms,
            );
        }
    }

    for (interaction, item) in familiar_list_interaction.iter_mut() {
        if *interaction == Interaction::Pressed {
            focus_list_entity(
                item.0,
                "familiar",
                &mut selected_entity,
                &mut q_camera,
                &q_transforms,
            );
        }
    }

    for (interaction, button, mut color) in familiar_max_soul_buttons.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(theme.colors.button_pressed);
                handle_familiar_max_soul_adjustment(
                    *button,
                    &mut q_familiar_ops,
                    &q_familiar_meta,
                    &node_index,
                    &mut q_text,
                    &mut ev_max_soul_changed,
                );
            }
            Interaction::Hovered => {
                *color = BackgroundColor(theme.colors.button_hover);
            }
            Interaction::None => {
                *color = BackgroundColor(theme.colors.button_default);
            }
        }
    }
}
