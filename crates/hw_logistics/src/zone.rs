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

pub const STOCKPILE_ACCEPTANCE_RESOURCES: [ResourceType; 9] = [
    ResourceType::Wood,
    ResourceType::Rock,
    ResourceType::Water,
    ResourceType::BucketEmpty,
    ResourceType::BucketWater,
    ResourceType::Sand,
    ResourceType::Bone,
    ResourceType::StasisMud,
    ResourceType::Wheelbarrow,
];

const STOCKPILE_ACCEPTANCE_ALL_BITS: u16 = (1 << STOCKPILE_ACCEPTANCE_RESOURCES.len()) - 1;

const fn stockpile_resource_bit(resource_type: ResourceType) -> u16 {
    1 << match resource_type {
        ResourceType::Wood => 0,
        ResourceType::Rock => 1,
        ResourceType::Water => 2,
        ResourceType::BucketEmpty => 3,
        ResourceType::BucketWater => 4,
        ResourceType::Sand => 5,
        ResourceType::Bone => 6,
        ResourceType::StasisMud => 7,
        ResourceType::Wheelbarrow => 8,
    }
}

/// Fixed-size durable set used by the player-managed stockpile checklist.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[reflect(Default)]
pub struct StockpileResourceSet {
    bits: u16,
}

impl StockpileResourceSet {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn all() -> Self {
        Self {
            bits: STOCKPILE_ACCEPTANCE_ALL_BITS,
        }
    }

    pub const fn contains(self, resource_type: ResourceType) -> bool {
        self.bits & stockpile_resource_bit(resource_type) != 0
    }

    pub const fn with(self, resource_type: ResourceType, accepted: bool) -> Self {
        let bit = stockpile_resource_bit(resource_type);
        let bits = if accepted {
            self.bits | bit
        } else {
            self.bits & !bit
        };
        Self {
            bits: bits & STOCKPILE_ACCEPTANCE_ALL_BITS,
        }
    }

    pub const fn len(self) -> usize {
        (self.bits & STOCKPILE_ACCEPTANCE_ALL_BITS).count_ones() as usize
    }

    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// A durable rule describing which resources a player-managed stockpile cell accepts.
///
/// `Any` and `Only` remain stable for existing v1 save bodies. New checklist edits use
/// `Selected` whenever zero or multiple-but-not-all resources are enabled.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockpileAcceptance {
    Any,
    Only(ResourceType),
    Selected(StockpileResourceSet),
}

impl StockpileAcceptance {
    pub const fn none() -> Self {
        Self::Selected(StockpileResourceSet::empty())
    }

    pub const fn resource_set(self) -> StockpileResourceSet {
        match self {
            Self::Any => StockpileResourceSet::all(),
            Self::Only(resource_type) => StockpileResourceSet::empty().with(resource_type, true),
            Self::Selected(resources) => StockpileResourceSet {
                bits: resources.bits & STOCKPILE_ACCEPTANCE_ALL_BITS,
            },
        }
    }

    pub const fn from_resource_set(resources: StockpileResourceSet) -> Self {
        let resources = StockpileResourceSet {
            bits: resources.bits & STOCKPILE_ACCEPTANCE_ALL_BITS,
        };
        match resources.bits {
            STOCKPILE_ACCEPTANCE_ALL_BITS => Self::Any,
            bits if bits == stockpile_resource_bit(ResourceType::Wood) => {
                Self::Only(ResourceType::Wood)
            }
            bits if bits == stockpile_resource_bit(ResourceType::Rock) => {
                Self::Only(ResourceType::Rock)
            }
            bits if bits == stockpile_resource_bit(ResourceType::Water) => {
                Self::Only(ResourceType::Water)
            }
            bits if bits == stockpile_resource_bit(ResourceType::BucketEmpty) => {
                Self::Only(ResourceType::BucketEmpty)
            }
            bits if bits == stockpile_resource_bit(ResourceType::BucketWater) => {
                Self::Only(ResourceType::BucketWater)
            }
            bits if bits == stockpile_resource_bit(ResourceType::Sand) => {
                Self::Only(ResourceType::Sand)
            }
            bits if bits == stockpile_resource_bit(ResourceType::Bone) => {
                Self::Only(ResourceType::Bone)
            }
            bits if bits == stockpile_resource_bit(ResourceType::StasisMud) => {
                Self::Only(ResourceType::StasisMud)
            }
            bits if bits == stockpile_resource_bit(ResourceType::Wheelbarrow) => {
                Self::Only(ResourceType::Wheelbarrow)
            }
            _ => Self::Selected(resources),
        }
    }

    pub const fn normalized(self) -> Self {
        Self::from_resource_set(self.resource_set())
    }

    pub const fn with_resource(self, resource_type: ResourceType, accepted: bool) -> Self {
        Self::from_resource_set(self.resource_set().with(resource_type, accepted))
    }

    pub const fn accepts(self, resource_type: ResourceType) -> bool {
        self.resource_set().contains(resource_type)
    }

    pub const fn allowed_count(self) -> usize {
        self.resource_set().len()
    }

    pub const fn is_all(self) -> bool {
        self.allowed_count() == STOCKPILE_ACCEPTANCE_RESOURCES.len()
    }

    pub const fn is_none(self) -> bool {
        self.resource_set().is_empty()
    }

    pub fn accepted_resources(self) -> impl Iterator<Item = ResourceType> {
        STOCKPILE_ACCEPTANCE_RESOURCES
            .into_iter()
            .filter(move |resource_type| self.accepts(*resource_type))
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
        self.acceptance = self.acceptance.normalized();
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
                acceptance: self.acceptance.unwrap_or(current.acceptance).normalized(),
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

    #[test]
    fn acceptance_checklist_supports_empty_multiple_single_and_all_sets() {
        let none = StockpileAcceptance::none();
        assert!(none.is_none());
        assert!(!none.accepts(ResourceType::Wood));

        let wood_and_rock = none
            .with_resource(ResourceType::Wood, true)
            .with_resource(ResourceType::Rock, true);
        assert_eq!(wood_and_rock.allowed_count(), 2);
        assert!(wood_and_rock.accepts(ResourceType::Wood));
        assert!(wood_and_rock.accepts(ResourceType::Rock));
        assert!(!wood_and_rock.accepts(ResourceType::Bone));
        assert!(matches!(wood_and_rock, StockpileAcceptance::Selected(_)));

        let wood_only = wood_and_rock.with_resource(ResourceType::Rock, false);
        assert_eq!(wood_only, StockpileAcceptance::Only(ResourceType::Wood));

        let all = STOCKPILE_ACCEPTANCE_RESOURCES
            .into_iter()
            .fold(StockpileAcceptance::none(), |acceptance, resource| {
                acceptance.with_resource(resource, true)
            });
        assert_eq!(all, StockpileAcceptance::Any);
        assert!(all.is_all());
    }

    #[test]
    fn legacy_acceptance_variants_keep_their_meaning() {
        assert!(StockpileAcceptance::Any.accepts(ResourceType::Wheelbarrow));
        assert!(StockpileAcceptance::Only(ResourceType::Bone).accepts(ResourceType::Bone));
        assert!(!StockpileAcceptance::Only(ResourceType::Bone).accepts(ResourceType::Wood));
    }
}
