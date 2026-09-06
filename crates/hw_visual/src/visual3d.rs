//! Scene RtT内の建物presentationとshared-pool Soul billboard用コンポーネント。

use bevy::prelude::*;
use std::collections::HashMap;

use crate::wall_connection::{
    QuarterTurns, ResolvedWallTopology, WallConnectionMask, WallMeshFamily,
};

/// 完成した Building エンティティに対応する独立3Dビジュアルエンティティのマーカー。
///
/// Building の 2D Transform を変更せず、XZ 平面上の独立エンティティとして配置される。
#[derive(Component, Debug, Clone)]
pub struct Building3dVisual {
    pub owner: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wall3dPresentationMode {
    #[default]
    Fallback,
    Production,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wall3dPresentationState {
    pub topology: ResolvedWallTopology,
    pub mode: Wall3dPresentationMode,
    pub activation_revision: u64,
}

impl Default for Wall3dPresentationState {
    fn default() -> Self {
        Self {
            topology: ResolvedWallTopology {
                family: WallMeshFamily::Isolated,
                quarter_turns_y: QuarterTurns::ZERO,
            },
            mode: Wall3dPresentationMode::Fallback,
            activation_revision: 0,
        }
    }
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DoorPresentationAxis {
    #[default]
    EastWest,
    NorthSouth,
}

impl DoorPresentationAxis {
    pub const fn quarter_turns_y(self) -> QuarterTurns {
        match self {
            Self::EastWest => QuarterTurns::ZERO,
            Self::NorthSouth => QuarterTurns::ONE,
        }
    }

    /// Cardinal direction encoded in `MeshTag` for the closed leaf normal.
    pub const fn mesh_tag_direction(self) -> u32 {
        match self {
            Self::EastWest => 0,
            Self::NorthSouth => 3,
        }
    }
}

pub const fn resolve_door_presentation_axis(mask: WallConnectionMask) -> DoorPresentationAxis {
    let bits = mask.bits();
    if bits & 0b0011 == 0b0011 {
        DoorPresentationAxis::EastWest
    } else if bits & 0b1100 == 0b1100 {
        DoorPresentationAxis::NorthSouth
    } else {
        DoorPresentationAxis::EastWest
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Door3dPresentationMode {
    #[default]
    Fallback,
    Production,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn door_axis_table_is_total_and_prefers_east_west() {
        for bits in 0_u8..16 {
            let mask = WallConnectionMask::from_neighbors(
                bits & 0b1000 != 0,
                bits & 0b0100 != 0,
                bits & 0b0010 != 0,
                bits & 0b0001 != 0,
            );
            let expected = if bits & 0b0011 == 0b0011 {
                DoorPresentationAxis::EastWest
            } else if bits & 0b1100 == 0b1100 {
                DoorPresentationAxis::NorthSouth
            } else {
                DoorPresentationAxis::EastWest
            };
            assert_eq!(resolve_door_presentation_axis(mask), expected);
        }
    }

    #[test]
    fn door_axis_fixes_rotation_and_mesh_tag_direction() {
        assert_eq!(
            DoorPresentationAxis::EastWest.quarter_turns_y(),
            QuarterTurns::ZERO
        );
        assert_eq!(DoorPresentationAxis::EastWest.mesh_tag_direction(), 0);
        assert_eq!(
            DoorPresentationAxis::NorthSouth.quarter_turns_y(),
            QuarterTurns::ONE
        );
        assert_eq!(DoorPresentationAxis::NorthSouth.mesh_tag_direction(), 3);
    }
}
