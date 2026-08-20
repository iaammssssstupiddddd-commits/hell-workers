//! 3D レンダリング用コンポーネント定義
//!
//! Phase 2 ハイブリッド RtT アーキテクチャで使用するプロキシコンポーネント群。
//! Phase 3 で GLB モデルに置き換えるまでのプレースホルダー実装。

use bevy::prelude::*;
use std::collections::HashMap;

/// 完成した Building エンティティに対応する独立3Dビジュアルエンティティのマーカー。
///
/// Building の 2D Transform を変更せず、XZ 平面上の独立エンティティとして配置される。
#[derive(Component, Debug, Clone)]
pub struct Building3dVisual {
    pub owner: Entity,
}

/// Door-specific active 3D presentation. The root `Door` remains the only
/// semantic writer; this component only identifies its visual consumer.
#[derive(Component, Debug, Clone)]
pub struct Door3dVisual {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DoorPresentationState {
    #[default]
    Closed,
    Open,
    Locked,
}

/// Stable finite semantic state consumed by structural 3D equipment visuals.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StructuralPresentationState {
    #[default]
    Neutral,
    TankEmpty,
    TankPartial,
    TankFull,
    MixerIdle,
    MixerActive,
}

/// Shared-pool alpha-masked Soul billboard participating in Scene depth.
#[derive(Component, Debug, Clone)]
pub struct ActorBillboard3d {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoulBillboardFrame {
    #[default]
    Normal,
    Exhausted,
    Happy,
    Sleep,
    Wine,
    Trump,
    Stress,
    StressBreakdown,
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

/// owner → active billboard entity の O(1) lookup cache.
#[derive(Resource, Default)]
pub struct ActorBillboardOwnerCache {
    pub actor_billboard: HashMap<Entity, Entity>,
}
