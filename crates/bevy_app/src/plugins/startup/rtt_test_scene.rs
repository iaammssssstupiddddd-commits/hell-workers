//! M4 検証用: RtT 合成スプライトとテスト3Dキューブ
//!
//! このモジュールは RtT パイプラインの目視確認のためのテスト実装である。
//! M4 検証完了後、Phase 2 開始前に削除すること。

use crate::plugins::startup::RttTextures;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use hw_core::constants::{LAYER_2D, LAYER_3D};
use hw_ui::camera::MainCamera;

/// RtT テクスチャを Camera2d の子エンティティとして合成表示する。
///
/// Camera2d の子にすることで、パン・ズームに自動追従する。
/// 子エンティティのスケールは親（Camera2d）のズームスケールを継承するため、
/// どのズームレベルでもスプライトがビューポートをカバーする。
/// Z=20.0 は既存の最前面コンテンツ（Z_SPEECH_BUBBLE=11.0）より手前。
pub fn spawn_rtt_composite_sprite(
    mut commands: Commands,
    rtt: Res<RttTextures>,
    q_cam2d: Query<Entity, With<MainCamera>>,
) {
    let Ok(cam2d_entity) = q_cam2d.single() else {
        return;
    };

    let sprite_entity = commands
        .spawn((
            Sprite::from_image(rtt.texture_3d.clone()),
            Transform::from_xyz(0.0, 0.0, 20.0),
            RenderLayers::layer(LAYER_2D),
        ))
        .id();

    commands.entity(cam2d_entity).add_child(sprite_entity);
}

/// テスト用3Dキューブを LAYER_3D に配置する。
/// Camera3d（RtT）が描画し、合成スプライト経由で画面に現れる。
pub fn spawn_test_cube_3d(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(50.0, 50.0, 50.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.2, 0.2),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RenderLayers::layer(LAYER_3D),
    ));
}
