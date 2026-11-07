//! 開発用デバッグパネル
//!
//! ロジック確認のための 3D 表示切り替えボタン・即時ビルドトグルを提供する。

use crate::assets::GameAssets;
use crate::systems::visual::terrain_lod::{LodLevel, TerrainLodMetrics, TerrainLodState};
use bevy::prelude::*;
use hw_core::quality::{QualitySettings, RttQualityPreset};
use hw_ui::components::{UiInputBlocker, UiMountSlot, UiNodeRegistry, UiSlot};
use hw_ui::theme::UiTheme;
use hw_ui::widgets::{TextFieldConfig, TextFieldRole, spawn_text_field};

mod actions;
mod components;
mod presentation;
mod spawn;

pub use actions::{
    toggle_dev_panel_minimize_button_system, toggle_instant_build_button_system,
    toggle_render3d_button_system, toggle_rtt_extra_light_button_system,
    toggle_rtt_light_button_system, toggle_rtt_scene_objects_button_system,
    toggle_rtt_terrain_button_system,
};
pub use components::{
    DevPanelBody, DevPanelMinimizeButton, DevPanelMinimizeButtonLabel, DevPanelRoot,
    InstantBuildButton, LodIndicatorText, RenderPerfStatusText, ToggleRender3dButton,
    ToggleRttExtraLightButton, ToggleRttLightButton, ToggleRttSceneObjectsButton,
    ToggleRttTerrainButton,
};
pub use presentation::{
    update_instant_build_button_visual_system, update_lod_indicator_system,
    update_render_perf_status_system, update_render3d_button_visual_system,
    update_rtt_extra_light_button_visual_system, update_rtt_light_button_visual_system,
    update_rtt_scene_objects_button_visual_system, update_rtt_terrain_button_visual_system,
};
pub use spawn::spawn_dev_panel_system;

pub fn dev_panel_visibility_system(
    debug: Res<crate::DebugVisible>,
    #[cfg(feature = "profiling")] art_session: Option<
        Res<crate::assets::building_asset_set::acceptance::BuildingArtSession>,
    >,
    mut panels: Query<&mut Node, With<DevPanelRoot>>,
) {
    // The development overlay covers the first specimen in the explicit art
    // fixture. Keep this panel-only rule after settings have applied, so saved
    // debug-gizmo preferences cannot reveal it on a later frame.
    #[cfg(feature = "profiling")]
    let hide_for_art = art_session.is_some();
    #[cfg(not(feature = "profiling"))]
    let hide_for_art = false;
    let display = if debug.0 && !hide_for_art {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut panels {
        if node.display != display {
            node.display = display;
        }
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::*;
    use crate::assets::building_asset_set::{
        BuildingAssetAuthority, BuildingAssetKind, BuildingAssetSetIdentity,
        acceptance::{BuildingArtSession, BuildingArtView},
    };

    #[test]
    fn explicit_art_session_hides_only_the_dev_panel_after_debug_is_reenabled() {
        let mut app = App::new();
        app.insert_resource(crate::DebugVisible(true));
        let panel = app.world_mut().spawn((DevPanelRoot, Node::default())).id();
        app.add_systems(Update, dev_panel_visibility_system);

        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().display,
            Display::Flex
        );

        app.insert_resource(BuildingArtSession {
            mode: "art-preview".into(),
            identity: BuildingAssetSetIdentity {
                kind: BuildingAssetKind::Tank,
                generation: 1,
                authority: BuildingAssetAuthority::ArtPreview,
                manifest_sha256: "a".repeat(64),
            },
            locator: "manifests/building-tank-v1.buildingset".into(),
            status_path: "/tmp/dev-panel-test-status".into(),
            nonce: "a".repeat(32),
            view: BuildingArtView::Near,
            variant: Default::default(),
            lighting: Default::default(),
            spa_mask_page: None,
        });
        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().display,
            Display::None
        );

        // This mirrors settings re-applying a persisted debug-gizmos=true value.
        app.world_mut().resource_mut::<crate::DebugVisible>().0 = true;
        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().display,
            Display::None
        );

        app.world_mut().remove_resource::<BuildingArtSession>();
        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().display,
            Display::Flex
        );
    }
}
