//! Top-down building and terrain visual test scene.
//!
//! ゲーム本体とは独立して、建物と地形の2D/3D整合を検証する。
//! 右側のメニューパネルに操作一覧と現在値を常時表示。[H] でパネルを折りたたむ。
//!
//! ```bash
//! python3 scripts/dev.py cargo -- run -p visual_test
//! ```

pub mod building;
pub mod hud;
pub mod input;
pub mod setup;
pub mod systems;
pub mod types;

use bevy::camera_controller::pan_camera::PanCameraPlugin;
use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

use types::{LocalRttCompositeMaterial, TestState};

struct VisualTestPlugin;

impl Plugin for VisualTestPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PanCameraPlugin,
            Material2dPlugin::<LocalRttCompositeMaterial>::default(),
        ))
        .init_resource::<TestState>()
        .add_systems(
            Startup,
            (
                setup::setup_scene,
                building::setup_world_map,
                building::setup_building_assets,
            ),
        )
        .add_systems(
            PreUpdate,
            systems::handle_panel_scroll.after(bevy::input::InputSystems),
        )
        .add_systems(Update, input::keyboard_input)
        .add_systems(Update, systems::handle_button_interactions)
        .add_systems(
            Update,
            ((
                systems::sync_test_rtt_to_window,
                systems::sync_test_camera3d,
                systems::apply_composite_sprite,
            )
                .chain()
                .after(input::keyboard_input)
                .after(systems::handle_button_interactions),),
        )
        .add_systems(
            Update,
            (
                building::update_building_cursor,
                hud::apply_menu_visibility,
                hud::update_button_states,
                hud::update_dynamic_texts,
            )
                .chain(),
        );
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Visual Test — TopDown Buildings".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VisualTestPlugin)
        .run();
}
