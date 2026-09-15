// エンティティリストのセクション折りたたみ操作（純UIロジック）

use crate::components::{
    EntityListSectionType, SectionFolded, SectionToggle, UiInputState, UnassignedFolded,
    UnassignedSoulSection,
};
use crate::theme::UiTheme;
use bevy::prelude::*;

type SectionToggleInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Ref<'static, Interaction>,
        &'static SectionToggle,
        &'static mut BackgroundColor,
    ),
    With<Button>,
>;

/// SectionToggle ボタンの押下/ホバーに応じて
/// 折りたたみ状態コンポーネントを追加/削除し、ボタン色を更新する
pub fn entity_list_section_toggle_system(
    mut commands: Commands,
    mut interaction_query: SectionToggleInteractionQuery,
    q_folded: Query<Has<SectionFolded>>,
    unassigned_folded_query: Query<(Entity, Has<UnassignedFolded>), With<UnassignedSoulSection>>,
    theme: Res<UiTheme>,
    ui_input_state: Res<UiInputState>,
) {
    if ui_input_state.world_input_captured {
        return;
    }
    for (entity, interaction, toggle, mut color) in interaction_query.iter_mut() {
        if interaction.is_changed() || theme.is_changed() {
            color.0 = match *interaction {
                Interaction::Pressed => theme.colors.section_toggle_pressed,
                Interaction::Hovered => theme.colors.button_hover,
                Interaction::None => theme.colors.button_default,
            };
        }
        if ui_input_state.button_activated(entity) {
            toggle_list_section(&mut commands, toggle.0, &q_folded, &unassigned_folded_query);
        }
    }
}

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
                    commands
                        .entity(unassigned_entity)
                        .remove::<UnassignedFolded>();
                } else {
                    commands.entity(unassigned_entity).insert(UnassignedFolded);
                }
                any_toggled = true;
            }
            if !any_toggled {
                bevy::log::warn!("LIST: UnassignedSoulSection not found for toggling!");
            }
        }
    }
}
