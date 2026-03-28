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

/// DamnedSoul エンティティに対応する shadow caster 専用 proxy のマーカー。
#[derive(Component, Debug, Clone)]
pub struct SoulShadowProxy3d {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone)]
pub struct SoulAnimationPlayer3d {
    pub owner: Entity,
    pub current_body: SoulBodyAnimState,
    pub walk_facing_right: Option<bool>,
    pub last_owner_pos: Option<Vec2>,
    pub walk_variant_lock_secs: f32,
}

#[derive(Component, Debug, Clone)]
pub struct SoulFaceMaterial3d {
    pub owner: Entity,
    pub material: Handle<crate::CharacterMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoulBodyAnimState {
    #[default]
    Idle,
    Walk,
    Work,
    Carry,
    Fear,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoulFaceState {
    #[default]
    Normal,
    Fear,
    Exhausted,
    Focused,
    Happy,
    Sleep,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoulAnimVisualState {
    pub body: SoulBodyAnimState,
    pub face: SoulFaceState,
}

/// Familiar エンティティに対応する3Dプロキシのマーカー。
#[derive(Component, Debug, Clone)]
pub struct FamiliarProxy3d {
    pub owner: Entity,
}
