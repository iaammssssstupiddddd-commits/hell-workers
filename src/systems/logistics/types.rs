use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ResourceType {
    Wood,
    Rock, // 旧Stone（岩採掘でのみ入手可能）
    Water,
    BucketEmpty,
    BucketWater,
    Sand,
    StasisMud,
    Wheelbarrow,
}

impl ResourceType {
    /// 手押し車に積載可能か
    pub fn is_loadable(&self) -> bool {
        match self {
            ResourceType::Water => false,
            ResourceType::BucketWater => false,
            ResourceType::BucketEmpty => false,
            ResourceType::Wheelbarrow => false,
            _ => true, // Wood, Rock, Sand, StasisMud
        }
    }

    /// 猫車運搬が必須の資源か
    pub fn requires_wheelbarrow(&self) -> bool {
        matches!(self, ResourceType::Sand | ResourceType::StasisMud)
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ResourceItem(pub ResourceType);

/// アイテムがタスク発行済み（占有中）であることを示す
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ReservedForTask;

/// エンティティが特定の親（タンクなど）に属することを示す
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct BelongsTo(pub Entity);

/// タンク用バケツ置き場スロットであることを示す
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct BucketStorage;

/// 手押し車コンポーネント
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct Wheelbarrow {
    pub capacity: usize,
}

/// 手押し車の駐車エリア
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct WheelbarrowParking {
    pub capacity: usize,
}

/// Tank Blueprint との一時リンク（完成時に BelongsTo へ昇格）
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct PendingBelongsToBlueprint(pub Entity);

/// ソウルが持っているアイテムのエンティティ
#[derive(Component, Default, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Inventory(pub Option<Entity>);

/// アイテムがストックパイルに格納されていることを示すコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct InStockpile(pub Entity);

/// アイテムがソウルに要求されていることを示すコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ClaimedBy(pub Entity);
