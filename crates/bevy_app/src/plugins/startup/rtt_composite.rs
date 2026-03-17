//! RtT 合成スプライト（恒久実装）
//!
//! Camera3d がオフスクリーンテクスチャに描画した3Dコンテンツを、
//! ワールド原点に固定した sprite として overlay camera 経由で合成する。
//!
//! composite sprite はワールド原点（0, 0, Z_RTT_COMPOSITE）に固定し、
//! overlay camera（固定位置）が LAYER_OVERLAY を描画することで常に全画面表示する。
//! 3D コンテンツのパン・ズーム追従は Camera3d の Transform が camera_sync で行う。

use crate::plugins::startup::RttTextures;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use hw_core::constants::{LAYER_OVERLAY, Z_RTT_COMPOSITE};

/// RtT composite sprite のマーカーコンポーネント。3D表示切り替えで可視性を制御する。
#[derive(Component)]
pub struct RttCompositeSprite;

/// RtT テクスチャをワールド原点に固定したスプライトとして合成表示する。
pub fn spawn_rtt_composite_sprite(
    mut commands: Commands,
    rtt: Res<RttTextures>,
) {
    commands.spawn((
        Sprite::from_image(rtt.texture_3d.clone()),
        Transform::from_xyz(0.0, 0.0, Z_RTT_COMPOSITE),
        RenderLayers::layer(LAYER_OVERLAY),
        RttCompositeSprite,
    ));
}

/// `RttTextures.texture_3d` が差し替わったとき合成スプライトのサイズを自動更新する。
/// ウィンドウリサイズ後の `on_window_resized`（MS-P3-Pre-B 本実装）で呼ばれることを想定。
pub fn sync_rtt_composite_sprite(
    rtt: Res<RttTextures>,
    images: Res<Assets<Image>>,
    mut sprites: Query<(&mut Sprite, &mut Transform), With<RttCompositeSprite>>,
) {
    if !rtt.is_changed() {
        return;
    }
    let Some(image) = images.get(&rtt.texture_3d) else {
        return;
    };
    let size = image.size_f32();
    for (mut sprite, mut tf) in sprites.iter_mut() {
        sprite.custom_size = Some(size);
        tf.translation.z = Z_RTT_COMPOSITE;
    }
}
