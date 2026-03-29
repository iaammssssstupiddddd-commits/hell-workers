//! Soul GLB Visual Test Scene
//!
//! ゲーム本体とは独立して、表情アトラス・モーション・Z-fight を検証する。
//! 右側のメニューパネルに操作一覧と現在値を常時表示。[H] でパネルを折りたたみ。
//!
//! ```bash
//! CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo run -p visual_test
//! ```

pub mod building;
pub mod hud;
pub mod input;
pub mod setup;
pub mod soul;
pub mod systems;
pub mod types;

use bevy::camera_controller::pan_camera::PanCameraPlugin;
use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use hw_visual::{CharacterMaterial, SoulMaskMaterial, SoulShadowMaterial};

use types::{LocalRttCompositeMaterial, TestElev, TestState};

struct VisualTestPlugin;

impl Plugin for VisualTestPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PanCameraPlugin,
            MaterialPlugin::<SoulShadowMaterial>::default(),
            MaterialPlugin::<SoulMaskMaterial>::default(),
            Material2dPlugin::<LocalRttCompositeMaterial>::default(),
        ))
        .init_resource::<TestState>()
        .init_resource::<TestElev>()
        .add_systems(
            Startup,
            (
                setup::setup_scene,
                building::setup_world_map,
                building::setup_building_assets,
            ),
        )
        .add_observer(soul::on_soul_scene_ready)
        .add_observer(soul::on_shadow_scene_ready)
        .add_observer(soul::on_mask_scene_ready)
        .add_systems(
            PreUpdate,
            systems::handle_panel_scroll.after(bevy::input::InputSystems),
        )
        .add_systems(
            Update,
            (
                input::keyboard_input,
                systems::handle_button_interactions,
                systems::sync_test_camera3d,
                soul::sync_mask_proxies,
                soul::sync_shadow_proxies,
                systems::apply_faces,
                systems::apply_motion,
                systems::apply_animation,
                systems::apply_shader_params,
                systems::apply_composite_sprite,
                building::update_building_cursor,
                hud::apply_menu_visibility,
                hud::update_section_visibility,
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
                title: "Visual Test — Soul GLB".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MaterialPlugin::<CharacterMaterial>::default())
        .add_plugins(VisualTestPlugin)
        .run();
}
