use bevy::prelude::*;
use hw_ui::UiIntent;
use hw_ui::camera::MainCamera;
use hw_ui::components::*;
use hw_ui::theme::UiTheme;

mod navigation;
mod visual {
    pub use hw_ui::list::{apply_row_highlight, entity_list_visual_feedback_system};
}

pub use hw_ui::list::entity_list_section_toggle_system;
pub use navigation::{
    entity_list_scroll_hint_visibility_system, entity_list_scroll_system,
    entity_list_tab_focus_system, update_unassigned_arrow_icon_system,
};
pub use visual::{apply_row_highlight, entity_list_visual_feedback_system};

fn focus_list_entity(
    entity: Entity,
    label: &'static str,
    selected_entity: &mut ResMut<crate::interface::selection::SelectedEntity>,
    q_camera: &mut Query<&mut Transform, With<MainCamera>>,
    q_transforms: &Query<&GlobalTransform>,
) {
    hw_ui::list::select_entity_and_focus_camera(
        entity,
        label,
        selected_entity,
        q_camera,
        q_transforms,
    );
}

/// エンティティリストのゲーム側インタラクション
/// （行クリック選択 + 使役数上限変更）
/// セクション折りたたみは `entity_list_section_toggle_system` (hw_ui) が担当する
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn entity_list_interaction_system(
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
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    mut q_camera: Query<&mut Transform, With<MainCamera>>,
    q_transforms: Query<&GlobalTransform>,
    mut ui_intents: MessageWriter<UiIntent>,
    theme: Res<UiTheme>,
) {
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
                ui_intents.write(UiIntent::AdjustMaxControlledSoulFor(
                    button.familiar,
                    button.delta,
                ));
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
