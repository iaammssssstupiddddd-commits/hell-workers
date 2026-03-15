//! RtT 合成スプライト（恒久実装）
//!
//! Camera3d がオフスクリーンテクスチャに描画した3Dコンテンツを、
//! ワールド原点に固定した sprite として overlay camera 経由で合成する。
//!
//! composite sprite はワールド原点（0, 0, 20）に固定し、
//! overlay camera（固定位置）が LAYER_OVERLAY を描画することで常に全画面表示する。
//! 3D コンテンツのパン・ズーム追従は Camera3d の Transform が camera_sync で行う。

use crate::plugins::startup::RttTextures;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use hw_core::constants::LAYER_OVERLAY;

/// RtT テクスチャをワールド原点に固定したスプライトとして合成表示する。
pub fn spawn_rtt_composite_sprite(
    mut commands: Commands,
    rtt: Res<RttTextures>,
) {
    commands.spawn((
        Sprite::from_image(rtt.texture_3d.clone()),
        Transform::from_xyz(0.0, 0.0, 20.0),
        RenderLayers::layer(LAYER_OVERLAY),
    ));
}
