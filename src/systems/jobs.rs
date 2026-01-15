use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;
use std::collections::HashMap;

// --- Events ---

#[derive(Message)]
pub struct DesignationCreatedEvent {
    pub entity: Entity,
    pub work_type: WorkType,
    pub issued_by: Option<Entity>, // None = 未アサイン
    pub priority: u32,
}

#[derive(Message)]
pub struct TaskCompletedEvent {
    pub _soul_entity: Entity,
    pub _task_type: WorkType,
}

// --- Components ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum BuildingType {
    Wall,
    Floor,
}

impl BuildingType {
    /// この建物タイプに必要な資材を返す
    pub fn required_materials(&self) -> HashMap<ResourceType, u32> {
        let mut materials = HashMap::new();
        match self {
            BuildingType::Wall => {
                materials.insert(ResourceType::Wood, 2);
            }
            BuildingType::Floor => {
                materials.insert(ResourceType::Stone, 1);
            }
        }
        materials
    }
}

#[derive(Component)]
pub struct Building {
    pub _kind: BuildingType,
}

/// 資材の運搬先となる Blueprint を示すマーカー
#[derive(Component)]
pub struct TargetBlueprint(pub Entity);

#[derive(Component)]
pub struct Tree;

#[derive(Component)]
pub struct Rock;

/// 設計図コンポーネント - 建設中の建物を表す
#[derive(Component)]
pub struct Blueprint {
    pub kind: BuildingType,
    /// 建築進捗 (0.0 to 1.0) - 資材が揃った後の建築作業の進捗
    pub progress: f32,
    /// 必要な資材量
    pub required_materials: HashMap<ResourceType, u32>,
    /// 搬入済みの資材量
    pub delivered_materials: HashMap<ResourceType, u32>,
}

impl Blueprint {
    /// 新しい設計図を作成
    pub fn new(kind: BuildingType) -> Self {
        Self {
            kind,
            progress: 0.0,
            required_materials: kind.required_materials(),
            delivered_materials: HashMap::new(),
        }
    }

    /// 資材が全て揃っているかチェック
    pub fn materials_complete(&self) -> bool {
        for (resource_type, required) in &self.required_materials {
            let delivered = self.delivered_materials.get(resource_type).unwrap_or(&0);
            if delivered < required {
                return false;
            }
        }
        true
    }

    pub fn deliver_material(&mut self, resource_type: ResourceType, amount: u32) {
        *self.delivered_materials.entry(resource_type).or_insert(0) += amount;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum WorkType {
    #[default]
    Chop, // 伐採
    Mine,  // 採掘
    Build, // 建築
    Haul,  // 運搬
}

#[derive(Component)]
pub struct Designation {
    pub work_type: WorkType,
}

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct TaskSlots {
    pub max: u32,
}

impl TaskSlots {
    pub fn new(max: u32) -> Self {
        Self { max }
    }
}

// IssuedBy は relationships.rs の ManagedBy に移行
// 後方互換性のため、エイリアスを提供
pub use crate::relationships::ManagedBy as IssuedBy;

// --- Systems ---

pub fn building_completion_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
) {
    for (entity, bp, transform) in q_blueprints.iter_mut() {
        // 資材が揃っていて、建築進捗が100%に達したら完成
        if bp.materials_complete() && bp.progress >= 1.0 {
            info!(
                "BUILDING: Completed at {:?} (materials: {:?})",
                transform.translation, bp.delivered_materials
            );
            commands.entity(entity).despawn();

            let sprite_image = match bp.kind {
                BuildingType::Wall => game_assets.wall.clone(),
                BuildingType::Floor => game_assets.stone.clone(),
            };

            commands.spawn((
                Building { _kind: bp.kind },
                Sprite {
                    image: sprite_image,
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                *transform,
                Name::new(format!("Building ({:?})", bp.kind)),
            ));
        }
    }
}
