use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ResourceType {
    Wood,
    Rock,
    Water,
    BucketEmpty,
    BucketWater,
    Sand,
    Bone,
    StasisMud,
    Wheelbarrow,
}

impl ResourceType {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Wood => "Wood",
            Self::Rock => "Rock",
            Self::Water => "Water",
            Self::BucketEmpty => "Empty Bucket",
            Self::BucketWater => "Water Bucket",
            Self::Sand => "Sand",
            Self::Bone => "Bone",
            Self::StasisMud => "Stasis Mud",
            Self::Wheelbarrow => "Wheelbarrow",
        }
    }

    pub fn is_loadable(&self) -> bool {
        !matches!(
            self,
            ResourceType::Water
                | ResourceType::BucketWater
                | ResourceType::BucketEmpty
                | ResourceType::Wheelbarrow
        )
    }

    pub fn requires_wheelbarrow(&self) -> bool {
        matches!(
            self,
            ResourceType::Sand | ResourceType::StasisMud | ResourceType::Bone
        )
    }

    pub fn can_store_in_stockpile(&self) -> bool {
        !matches!(
            self,
            ResourceType::Sand | ResourceType::Bone | ResourceType::StasisMud
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum WheelbarrowDestination {
    Stockpile(Entity),
    Blueprint(Entity),
    Mixer {
        entity: Entity,
        resource_type: ResourceType,
    },
}

impl WheelbarrowDestination {
    pub fn entity(self) -> Entity {
        match self {
            Self::Stockpile(entity) | Self::Blueprint(entity) => entity,
            Self::Mixer { entity, .. } => entity,
        }
    }

    pub fn stockpile_or_blueprint(self) -> Option<Entity> {
        match self {
            Self::Stockpile(entity) | Self::Blueprint(entity) => Some(entity),
            Self::Mixer { .. } => None,
        }
    }
}
