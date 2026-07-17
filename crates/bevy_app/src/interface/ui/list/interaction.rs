use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::UiIntent;
use hw_ui::camera::MainCamera;
use hw_ui::components::*;
use hw_ui::theme::UiTheme;

type SoulListInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static SoulListItem),
    (
        Changed<Interaction>,
        With<Button>,
        Without<FamiliarListItem>,
    ),
>;

type FamiliarListInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static FamiliarListItem),
    (Changed<Interaction>, With<Button>, Without<SoulListItem>),
>;

type FamiliarMaxSoulButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static FamiliarMaxSoulAdjustButton,
        &'static mut BackgroundColor,
    ),
    (
        Changed<Interaction>,
        With<Button>,
        Without<FamiliarListItem>,
        Without<SoulListItem>,
        Without<SectionToggle>,
    ),
>;

mod navigation;
mod visual {
    pub use hw_ui::list::{apply_row_highlight, entity_list_visual_feedback_system};
}

pub use hw_ui::list::entity_list_section_toggle_system;
pub use navigation::{
    entity_list_scroll_hint_visibility_system, entity_list_tab_focus_system,
    update_unassigned_arrow_icon_system,
};
pub use visual::{apply_row_highlight, entity_list_visual_feedback_system};

#[derive(SystemParam)]
pub struct FocusQueries<'w, 's> {
    q_camera: Query<'w, 's, &'static mut Transform, With<MainCamera>>,
    q_transforms: Query<'w, 's, &'static GlobalTransform>,
}

#[derive(SystemParam)]
pub struct EntityListInteractionResources<'w> {
    selected_entity: ResMut<'w, crate::interface::selection::SelectedEntity>,
    ui_intents: MessageWriter<'w, UiIntent>,
    theme: Res<'w, UiTheme>,
    resolved_frame: Res<'w, crate::input_actions::ResolvedInputFrame>,
}

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
pub fn entity_list_interaction_system(
    mut soul_list_interaction: SoulListInteractionQuery<'_, '_>,
    mut familiar_list_interaction: FamiliarListInteractionQuery<'_, '_>,
    mut familiar_max_soul_buttons: FamiliarMaxSoulButtonQuery<'_, '_>,
    mut focus_queries: FocusQueries,
    resources: EntityListInteractionResources,
) {
    let EntityListInteractionResources {
        mut selected_entity,
        mut ui_intents,
        theme,
        resolved_frame,
    } = resources;
    if !resolved_frame.pointer_selection_suppressed() {
        for (interaction, item) in soul_list_interaction.iter_mut() {
            if *interaction == Interaction::Pressed {
                focus_list_entity(
                    item.0,
                    "soul",
                    &mut selected_entity,
                    &mut focus_queries.q_camera,
                    &focus_queries.q_transforms,
                );
            }
        }

        for (interaction, item) in familiar_list_interaction.iter_mut() {
            if *interaction == Interaction::Pressed {
                focus_list_entity(
                    item.0,
                    "familiar",
                    &mut selected_entity,
                    &mut focus_queries.q_camera,
                    &focus_queries.q_transforms,
                );
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_contexts::TaskContext;
    use crate::entities::familiar::Familiar;
    use crate::input_actions::{
        InputAction, InputModifiers, InputPreUpdateSet, ResolvedInputFrame,
        configure_input_resolution_sets, resolve_input_frame_system,
    };
    use crate::systems::GameSystemSet;
    use crate::systems::command::{TaskMode, familiar_command_input_system};
    use crate::test_support::minimal_app;

    #[test]
    fn resolved_action_suppresses_entity_list_row_selection() {
        let mut app = minimal_app();
        app.add_message::<UiIntent>()
            .init_resource::<crate::interface::selection::SelectedEntity>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<UiTheme>()
            .add_systems(Update, entity_list_interaction_system);
        let target = app.world_mut().spawn(GlobalTransform::default()).id();
        app.world_mut()
            .spawn((Interaction::Pressed, Button, SoulListItem(target)));
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::FamiliarChop],
                None,
                true,
            );

        app.update();

        assert!(
            app.world()
                .resource::<crate::interface::selection::SelectedEntity>()
                .0
                .is_none()
        );
    }

    #[test]
    fn resolved_familiar_command_keeps_frame_target_during_same_frame_row_click() {
        let mut app = minimal_app();
        app.add_message::<UiIntent>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<UiInputState>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<TaskContext>()
            .init_resource::<MenuState>()
            .init_resource::<crate::interface::selection::SelectedEntity>()
            .init_resource::<crate::DebugVisible>()
            .init_resource::<Time<Virtual>>()
            .init_resource::<UiTheme>()
            .insert_resource(State::new(hw_core::game_state::PlayMode::Normal))
            .init_resource::<NextState<hw_core::game_state::PlayMode>>();
        configure_input_resolution_sets(&mut app);
        app.configure_sets(
            Update,
            (
                GameSystemSet::Input,
                GameSystemSet::Logic,
                GameSystemSet::Interface,
            )
                .chain(),
        );
        app.add_systems(
            PreUpdate,
            resolve_input_frame_system.in_set(InputPreUpdateSet::Resolve),
        );
        app.add_systems(
            Update,
            familiar_command_input_system.in_set(GameSystemSet::Logic),
        );
        app.add_systems(
            Update,
            entity_list_interaction_system.in_set(GameSystemSet::Interface),
        );

        let familiar = app.world_mut().spawn(Familiar::default()).id();
        let clicked_soul = app.world_mut().spawn(GlobalTransform::default()).id();
        app.world_mut()
            .spawn((Interaction::Pressed, Button, SoulListItem(clicked_soul)));
        app.world_mut()
            .resource_mut::<crate::interface::selection::SelectedEntity>()
            .0 = Some(familiar);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyC);

        app.update();

        assert_eq!(
            app.world().resource::<TaskContext>().0,
            TaskMode::DesignateChop(None)
        );
        assert_eq!(
            app.world()
                .resource::<crate::interface::selection::SelectedEntity>()
                .0,
            Some(familiar)
        );
    }
}
