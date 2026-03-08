use bevy::prelude::*;
pub use hw_core::logistics::ResourceType;

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
