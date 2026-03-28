//! 3D レンダリング用コンポーネント定義
//!
//! Phase 2 ハイブリッド RtT アーキテクチャで使用するプロキシコンポーネント群。
//! Phase 3 で GLB モデルに置き換えるまでのプレースホルダー実装。

use bevy::prelude::*;

/// 完成した Building エンティティに対応する独立3Dビジュアルエンティティのマーカー。
///
/// Building の 2D Transform を変更せず、XZ 平面上の独立エンティティとして配置される。
#[derive(Component, Debug, Clone)]
pub struct Building3dVisual {
    pub owner: Entity,
}

/// DamnedSoul エンティティに対応する3Dプロキシのマーカー。
#[derive(Component, Debug, Clone)]
pub struct SoulProxy3d {
    pub owner: Entity,
    pub billboard: bool,
}

/// DamnedSoul エンティティに対応する Soul 専用 mask プロキシのマーカー。
#[derive(Component, Debug, Clone)]
pub struct SoulMaskProxy3d {
    pub owner: Entity,
}

/// Familiar エンティティに対応する3Dプロキシのマーカー。
#[derive(Component, Debug, Clone)]
pub struct FamiliarProxy3d {
    pub owner: Entity,
}
