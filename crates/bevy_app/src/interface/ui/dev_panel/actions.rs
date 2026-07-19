use super::*;
use bevy::input_focus::InputFocus;

fn belongs_to_ui_subtree(entity: Entity, root: Entity, q_parents: &Query<&ChildOf>) -> bool {
    let mut current = entity;
    for _ in 0..64 {
        if current == root {
            return true;
        }
        let Ok(parent) = q_parents.get(current) else {
            return false;
        };
        current = parent.parent();
    }
    false
}

/// DevPanel 本文の最小化・復元を処理
pub fn toggle_dev_panel_minimize_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<DevPanelMinimizeButton>)>,
    mut q_body: Query<(Entity, &mut Node), With<DevPanelBody>>,
    mut q_label: Query<&mut Text, With<DevPanelMinimizeButtonLabel>>,
    q_parents: Query<&ChildOf>,
    mut input_focus: ResMut<InputFocus>,
) {
    if !q_button
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        return;
    }

    let Ok((body_entity, mut body)) = q_body.single_mut() else {
        return;
    };
    let Ok(mut label) = q_label.single_mut() else {
        return;
    };

    if body.display == Display::None {
        body.display = Display::Flex;
        label.0 = "-".to_string();
    } else {
        if input_focus
            .get()
            .is_some_and(|focused| belongs_to_ui_subtree(focused, body_entity, &q_parents))
        {
            input_focus.clear();
        }
        body.display = Display::None;
        label.0 = "+".to_string();
    }
}

/// 3D表示ボタンのクリックを処理
pub fn toggle_render3d_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRender3dButton>)>,
    mut render3d: ResMut<crate::Render3dVisible>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            render3d.0 = !render3d.0;
        }
    }
}

/// 即時ビルドボタンのクリックを処理
pub fn toggle_instant_build_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<InstantBuildButton>)>,
    mut instant_build: ResMut<crate::DebugInstantBuild>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            instant_build.0 = !instant_build.0;
        }
    }
}

/// Soul mask ボタンのクリックを処理
pub fn toggle_soul_mask_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleSoulMaskButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.soul_mask_enabled = !perf_toggles.soul_mask_enabled;
        }
    }
}

/// RtT light ボタンのクリックを処理
pub fn toggle_rtt_light_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttLightButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.directional_light_enabled = !perf_toggles.directional_light_enabled;
        }
    }
}

/// 追加 RtT light ボタンのクリックを処理
pub fn toggle_rtt_extra_light_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttExtraLightButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.extra_directional_light_enabled =
                !perf_toggles.extra_directional_light_enabled;
        }
    }
}

/// RtT terrain ボタンのクリックを処理
pub fn toggle_rtt_terrain_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttTerrainButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.terrain_enabled = !perf_toggles.terrain_enabled;
        }
    }
}

/// RtT scene object ボタンのクリックを処理
pub fn toggle_rtt_scene_objects_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttSceneObjectsButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.scene_objects_enabled = !perf_toggles.scene_objects_enabled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimize_button_hides_and_restores_dev_panel_body() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>()
            .add_systems(Update, toggle_dev_panel_minimize_button_system);

        let button = app
            .world_mut()
            .spawn((Interaction::None, DevPanelMinimizeButton))
            .id();
        let body = app.world_mut().spawn((Node::default(), DevPanelBody)).id();
        let label = app
            .world_mut()
            .spawn((Text::new("-"), DevPanelMinimizeButtonLabel))
            .id();

        app.update();
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();

        assert_eq!(
            app.world().get::<Node>(body).unwrap().display,
            Display::None
        );
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "+");

        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
        app.update();
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();

        assert_eq!(
            app.world().get::<Node>(body).unwrap().display,
            Display::Flex
        );
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "-");
    }

    #[test]
    fn minimizing_clears_focus_inside_the_dev_panel_body() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>()
            .add_systems(Update, toggle_dev_panel_minimize_button_system);

        app.world_mut()
            .spawn((Interaction::Pressed, DevPanelMinimizeButton));
        let body = app.world_mut().spawn((Node::default(), DevPanelBody)).id();
        let field_root = app.world_mut().spawn((Node::default(), ChildOf(body))).id();
        let editable = app
            .world_mut()
            .spawn((Text::new("focused"), ChildOf(field_root)))
            .id();
        app.world_mut()
            .spawn((Text::new("-"), DevPanelMinimizeButtonLabel));
        app.insert_resource(InputFocus::from_entity(editable));

        app.update();

        assert_eq!(
            app.world().get::<Node>(body).unwrap().display,
            Display::None
        );
        assert!(app.world().resource::<InputFocus>().get().is_none());
    }
}
