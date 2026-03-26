use bevy::prelude::*;

use crate::transport_request::kinds::TransportRequestKind;
use crate::types::ResourceType;

pub use hw_core::logistics::WheelbarrowDestination;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Default)]
pub enum TransportPriority {
    Low = 0,
    #[default]
    Normal = 10,
    High = 20,
    Critical = 30,
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TransportRequest {
    pub kind: TransportRequestKind,
    pub anchor: Entity,
    pub resource_type: ResourceType,
    pub issued_by: Entity,
    pub priority: TransportPriority,
    pub stockpile_group: Vec<Entity>,
}

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct ManualTransportRequest;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct TransportRequestFixedSource(pub Entity);

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct ManualHaulPinnedSource;

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TransportDemand {
    pub desired_slots: u32,
    pub inflight: u32,
}

impl TransportDemand {
    pub fn remaining(&self) -> u32 {
        self.desired_slots.saturating_sub(self.inflight)
    }

    pub fn is_satisfied(&self) -> bool {
        self.remaining() == 0
    }
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TransportPolicy {
    pub allow_cross_area_source: bool,
    pub allow_cross_familiar_claim: bool,
    pub source_search_radius_tiles: f32,
}

impl Default for TransportPolicy {
    fn default() -> Self {
        Self {
            allow_cross_area_source: false,
            allow_cross_familiar_claim: false,
            source_search_radius_tiles: 20.0,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct WheelbarrowPendingSince(pub f64);

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct WheelbarrowLease {
    pub wheelbarrow: Entity,
    pub items: Vec<Entity>,
    pub source_pos: Vec2,
    pub destination: WheelbarrowDestination,
    pub lease_until: f64,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
#[derive(Default)]
pub enum TransportRequestState {
    #[default]
    Pending,
    Claimed,
}
