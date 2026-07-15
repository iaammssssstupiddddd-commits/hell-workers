use bevy::prelude::*;

pub use hw_core::logistics::ResourceType;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ResourceItem(pub ResourceType);

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
/// v0 save body を読む間だけ登録する compatibility shim。
///
/// この型の TypePath は既存 save format の一部なので、v0 support 中は移動・改名しない。
/// runtime code と v1 schema はこの marker を使用しない。
#[doc(hidden)]
pub struct ReservedForTask;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct BelongsTo(#[entities] pub Entity);

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct BucketStorage;

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct Wheelbarrow {
    pub capacity: usize,
}

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct WheelbarrowParking {
    pub capacity: usize,
}

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct PendingBelongsToBlueprint(#[entities] pub Entity);

#[derive(Component, Default, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Inventory(#[entities] pub Option<Entity>);
