use super::*;

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
