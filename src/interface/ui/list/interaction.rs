use crate::interface::ui::components::*;
use crate::interface::ui::theme::*;
use bevy::prelude::*;

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
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    mut q_camera: Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: Query<&GlobalTransform>,
    q_folded: Query<Has<SectionFolded>>,
    unassigned_folded_query: Query<(Entity, Has<UnassignedFolded>), With<UnassignedSoulSection>>,
) {
    for (interaction, toggle, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(COLOR_SECTION_TOGGLE_PRESSED);
                match toggle.0 {
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
            _ => {}
        }
    }

    for (interaction, item) in soul_list_interaction.iter_mut() {
        if *interaction == Interaction::Pressed {
            super::helpers::select_entity_and_focus_camera(
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
            super::helpers::select_entity_and_focus_camera(
                item.0,
                "familiar",
                &mut selected_entity,
                &mut q_camera,
                &q_transforms,
            );
        }
    }
}

/// 未所属ソウルセクションの矢印アイコンを折りたたみ状態に応じて更新
pub fn update_unassigned_arrow_icon_system(
    game_assets: Res<crate::assets::GameAssets>,
    unassigned_folded_query: Query<
        Has<UnassignedFolded>,
        (With<UnassignedSoulSection>, Changed<UnassignedFolded>),
    >,
    mut q_arrow: Query<&mut ImageNode, With<UnassignedSectionArrowIcon>>,
) {
    if let Some(is_folded) = unassigned_folded_query.iter().next() {
        for mut icon in q_arrow.iter_mut() {
            icon.image = if is_folded {
                game_assets.icon_arrow_right.clone()
            } else {
                game_assets.icon_arrow_down.clone()
            };
        }
    }
}
