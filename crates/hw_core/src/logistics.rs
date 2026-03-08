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
    pub fn is_loadable(&self) -> bool {
        match self {
            ResourceType::Water => false,
            ResourceType::BucketWater => false,
            ResourceType::BucketEmpty => false,
            ResourceType::Wheelbarrow => false,
            _ => true,
        }
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
