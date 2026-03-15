//! RtT 合成スプライト（恒久実装）
//!
//! Camera3d がオフスクリーンテクスチャに描画した3Dコンテンツを、
//! Camera2d の子エンティティとして2Dシーンに合成する。
//!
//! Camera2d の子にすることで、パン・ズームに自動追従する。
//! 子エンティティのスケールは親（Camera2d）のズームスケールを継承するため、
//! どのズームレベルでもスプライトがビューポートをカバーする。
//! Z=20.0 は既存の最前面コンテンツ（Z_SPEECH_BUBBLE=11.0）より手前。

use crate::plugins::startup::RttTextures;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use hw_core::constants::LAYER_2D;
use hw_ui::camera::MainCamera;

/// RtT テクスチャを Camera2d の子エンティティとして合成表示する。
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
