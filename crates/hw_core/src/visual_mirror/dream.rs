use bevy::prelude::*;

/// Dream アイコンの吸収アニメーション状態
///
/// UI アイコンにドリームパーティクルが吸収される際の
/// 視覚フィードバック（サイズ・色のパルス）を管理する。
#[derive(Component, Default)]
pub struct DreamIconAbsorb {
    pub timer: f32,
    pub pulse_count: u8,
}
