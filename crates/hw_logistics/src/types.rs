use bevy::prelude::*;

pub use hw_core::logistics::ResourceType;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ResourceItem(pub ResourceType);

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ReservedForTask;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct BelongsTo(pub Entity);

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
pub struct PendingBelongsToBlueprint(pub Entity);

#[derive(Component, Default, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Inventory(pub Option<Entity>);
