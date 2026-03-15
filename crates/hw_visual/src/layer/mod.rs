use bevy::prelude::*;

/// 建築物ビジュアルレイヤーの種別。
///
/// 親 `Building` エンティティの子として生成され、`Sprite` を保持する。
/// Phase 2 以降で `RenderLayers::layer(1)` を付与するだけで 3D 側へ移行できる。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualLayerKind {
    /// 床・地面面（Z_BUILDING_FLOOR = 0.05）
    Floor,
    /// 壁・構造体（Z_BUILDING_STRUCT = 0.12）
    Struct,
    /// 装飾レイヤー（Z_BUILDING_DECO = 0.15）
    Deco,
    /// 照明・エフェクト（Z_BUILDING_LIGHT = 0.18）
    Light,
}
