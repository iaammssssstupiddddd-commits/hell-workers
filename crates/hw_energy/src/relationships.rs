use bevy::prelude::*;

// ----- GeneratesFor / GridGenerators -----

/// SoulSpaSite → PowerGrid。発電機としてグリッドに登録する。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = GridGenerators)]
pub struct GeneratesFor(pub Entity);

impl Default for GeneratesFor {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// GeneratesFor の自動管理逆参照。PowerGrid エンティティ上に付与される。
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = GeneratesFor)]
pub struct GridGenerators(Vec<Entity>);

impl GridGenerators {
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ----- ConsumesFrom / GridConsumers -----

/// OutdoorLamp 等 → PowerGrid。消費者としてグリッドに登録する。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = GridConsumers)]
pub struct ConsumesFrom(pub Entity);

impl Default for ConsumesFrom {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// ConsumesFrom の自動管理逆参照。PowerGrid エンティティ上に付与される。
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = ConsumesFrom)]
pub struct GridConsumers(Vec<Entity>);

impl GridConsumers {
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
