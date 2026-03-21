//! RtT 合成スプライト（恒久実装）
//!
//! Camera3d がオフスクリーンテクスチャに描画した3Dコンテンツを、
//! ワールド原点に固定した sprite として overlay camera 経由で合成する。
//!
//! composite sprite はワールド原点（0, 0, Z_RTT_COMPOSITE）に固定し、
//! overlay camera（固定位置）が LAYER_OVERLAY を描画することで常に全画面表示する。
//! 3D コンテンツのパン・ズーム追従は Camera3d の Transform が camera_sync で行う。

use crate::plugins::startup::{Camera3dRtt, RttTextures};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::{LAYER_OVERLAY, Z_RTT_COMPOSITE, topdown_rtt_vertical_compensation};

/// RtT composite sprite のマーカーコンポーネント。3D表示切り替えで可視性を制御する。
#[derive(Component)]
pub struct RttCompositeSprite;

/// RtT テクスチャをワールド原点に固定したスプライトとして合成表示する。
pub fn spawn_rtt_composite_sprite(
    mut commands: Commands,
    rtt: Res<RttTextures>,
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    let custom_size = q_window.single().ok().map(logical_composite_size);
    commands.spawn((
        Sprite {
            image: rtt.texture_3d.clone(),
            custom_size,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, Z_RTT_COMPOSITE),
        RenderLayers::layer(LAYER_OVERLAY),
        RttCompositeSprite,
    ));
}

/// `RttTextures.texture_3d` が差し替わったとき、Camera3d の出力先と合成スプライトを同期する。
pub fn sync_rtt_output_bindings(
    rtt: Res<RttTextures>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut camera_targets: Query<&mut RenderTarget, With<Camera3dRtt>>,
    mut sprites: Query<(&mut Sprite, &mut Transform), With<RttCompositeSprite>>,
) {
    if !rtt.is_changed() {
        return;
    }
    if let Ok(mut target) = camera_targets.single_mut() {
        *target = RenderTarget::Image(rtt.texture_3d.clone().into());
    }

    let logical_size = q_window.single().ok().map(logical_composite_size);
    for (mut sprite, mut tf) in sprites.iter_mut() {
        sprite.image = rtt.texture_3d.clone();
        sprite.custom_size = logical_size;
        tf.translation.z = Z_RTT_COMPOSITE;
    }
}

fn logical_composite_size(window: &Window) -> Vec2 {
    let size = window.size();
    Vec2::new(size.x, size.y * topdown_rtt_vertical_compensation())
}
