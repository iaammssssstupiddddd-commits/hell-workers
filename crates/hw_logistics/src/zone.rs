use bevy::prelude::*;

use crate::transport_request::TransportPriority;
use crate::types::ResourceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ZoneType {
    Stockpile,
    Yard,
}

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Stockpile {
    pub capacity: usize,
    pub resource_type: Option<ResourceType>,
}

/// A durable rule describing which resource a player-managed stockpile cell accepts.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockpileAcceptance {
    Any,
    Only(ResourceType),
}

impl StockpileAcceptance {
    pub fn accepts(self, resource_type: ResourceType) -> bool {
        match self {
            Self::Any => true,
            Self::Only(accepted) => accepted == resource_type,
        }
    }
}

/// Durable player policy for an ordinary Yard-owned stockpile cell.
///
/// Special storage which happens to reuse [`Stockpile`] does not carry this component.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct StockpilePolicy {
    pub acceptance: StockpileAcceptance,
    pub inbound_priority: TransportPriority,
    pub target_amount: usize,
    pub allow_export: bool,
}

impl StockpilePolicy {
    /// Builds the compatibility policy for a cell with the given physical capacity.
    pub fn for_capacity(capacity: usize) -> Self {
        Self {
            acceptance: StockpileAcceptance::Any,
            inbound_priority: TransportPriority::Normal,
            target_amount: capacity,
            allow_export: true,
        }
    }

    /// Keeps a persisted or edited target within the cell's physical capacity.
    pub fn normalized_for_capacity(mut self, capacity: usize) -> Self {
        self.target_amount = self.target_amount.min(capacity);
        self
    }
}

/// Partial update shared by single-cell and range policy editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StockpilePolicyPatch {
    pub acceptance: Option<StockpileAcceptance>,
    pub inbound_priority: Option<TransportPriority>,
    pub target_amount: Option<usize>,
    pub allow_export: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockpilePolicyPatchResult {
    pub policy: StockpilePolicy,
    pub target_clamped: bool,
}

impl StockpilePolicyPatch {
    pub fn apply(self, current: StockpilePolicy, capacity: usize) -> StockpilePolicyPatchResult {
        let requested_target = self.target_amount.unwrap_or(current.target_amount);
        StockpilePolicyPatchResult {
            policy: StockpilePolicy {
                acceptance: self.acceptance.unwrap_or(current.acceptance),
                inbound_priority: self.inbound_priority.unwrap_or(current.inbound_priority),
                target_amount: requested_target.min(capacity),
                allow_export: self.allow_export.unwrap_or(current.allow_export),
            },
            target_clamped: requested_target > capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_policy_tracks_cell_capacity() {
        assert_eq!(
            StockpilePolicy::for_capacity(7),
            StockpilePolicy {
                acceptance: StockpileAcceptance::Any,
                inbound_priority: TransportPriority::Normal,
                target_amount: 7,
                allow_export: true,
            }
        );
    }

    #[test]
    fn policy_patch_clamps_only_the_target_and_preserves_other_fields() {
        let current = StockpilePolicy {
            acceptance: StockpileAcceptance::Only(ResourceType::Wood),
            inbound_priority: TransportPriority::High,
            target_amount: 4,
            allow_export: false,
        };

        let result = StockpilePolicyPatch {
            target_amount: Some(99),
            ..default()
        }
        .apply(current, 10);

        assert!(result.target_clamped);
        assert_eq!(result.policy.target_amount, 10);
        assert_eq!(result.policy.acceptance, current.acceptance);
        assert_eq!(result.policy.inbound_priority, current.inbound_priority);
        assert_eq!(result.policy.allow_export, current.allow_export);
    }
}
