use crate::systems::logistics::ResourceType;
use bevy::prelude::*;
use std::collections::HashMap;
mod building_completion;
pub mod floor_construction;
mod mud_mixer;
pub use building_completion::building_completion_system;
pub use floor_construction::*;
pub use mud_mixer::*;

// --- Events ---

// --- Components ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum BuildingType {
    #[default]
    Wall,
    Floor,
    Tank,
    MudMixer,
    RestArea,
    SandPile,
    BonePile,
    WheelbarrowParking,
}

impl BuildingType {
    /// この建物タイプに必要な資材を返す
    pub fn required_materials(&self) -> HashMap<ResourceType, u32> {
        let mut materials = HashMap::new();
        match self {
            BuildingType::Wall => {
                materials.insert(ResourceType::Wood, 1);
                materials.insert(ResourceType::StasisMud, 1);
            }
            BuildingType::Floor => {
                // Drag方式で建設されるため、Blueprintベースの資材搬入は不要
            }
            BuildingType::Tank => {
                materials.insert(ResourceType::Wood, 2);
            }
            BuildingType::MudMixer => {
                materials.insert(ResourceType::Wood, 4);
            }
            BuildingType::RestArea => {
                materials.insert(ResourceType::Wood, 5);
            }
            BuildingType::SandPile => {
                materials.insert(ResourceType::Sand, 10);
            }
            BuildingType::BonePile => {
                materials.insert(ResourceType::Bone, 10);
            }
            BuildingType::WheelbarrowParking => {
                materials.insert(ResourceType::Wood, 2);
            }
        }
        materials
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Building {
    pub kind: BuildingType,
    pub is_provisional: bool,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ProvisionalWall {
    pub mud_delivered: bool,
}

#[derive(Component)]
pub struct SandPile;

#[derive(Component)]
pub struct BonePile;

#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct RestArea {
    pub capacity: usize,
}

#[derive(Component)]
pub struct TargetBlueprint(pub Entity);

#[derive(Component)]
pub struct Tree;

#[derive(Component, Clone, Copy, Debug)]
pub struct TreeVariant(pub usize);

#[derive(Component)]
pub struct Rock;

/// 障害物のグリッド座標を保持するコンポーネント
#[derive(Component)]
pub struct ObstaclePosition(pub i32, pub i32);

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
    /// 占有するグリッド座標リスト
    pub occupied_grids: Vec<(i32, i32)>,
}

impl Blueprint {
    /// 新しい設計図を作成
    pub fn new(kind: BuildingType, occupied_grids: Vec<(i32, i32)>) -> Self {
        Self {
            kind,
            progress: 0.0,
            required_materials: kind.required_materials(),
            delivered_materials: HashMap::new(),
            occupied_grids,
        }
    }

    /// 資材が全て揃っているかチェック
    pub fn materials_complete(&self) -> bool {
        // 壁の場合、木材さえあれば建築作業開始は可能とする（仮設状態になる）
        if self.kind == BuildingType::Wall {
            let wood_delivered = self
                .delivered_materials
                .get(&ResourceType::Wood)
                .unwrap_or(&0);
            let wood_required = self
                .required_materials
                .get(&ResourceType::Wood)
                .unwrap_or(&1);
            return wood_delivered >= wood_required;
        }

        for (resource_type, required) in &self.required_materials {
            let delivered = self.delivered_materials.get(resource_type).unwrap_or(&0);
            if delivered < required {
                return false;
            }
        }
        true
    }

    /// 本来の全資材が揃っているか（仮設ではなく完全な状態か）
    pub fn is_fully_complete(&self) -> bool {
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
    Mine,               // 採掘
    Build,              // 建築
    Haul,               // 運搬（Stockpile行き）
    HaulToMixer,        // 固体原料（Sand/Rock）をミキサーへ運ぶ
    GatherWater,        // 水汲み
    CollectSand,        // 砂採取
    CollectBone,        // 骨採取
    Refine,             // 精製
    HaulWaterToMixer,   // Tankから水をミキサーへ運ぶ
    WheelbarrowHaul,    // 手押し車で一括運搬
    ReinforceFloorTile, // 床タイルの骨補強
    PourFloorTile,      // 床タイルへの泥注入
    CoatWall,           // 仮設壁への泥塗布
}

#[derive(Component)]
pub struct Designation {
    pub work_type: WorkType,
}

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Priority(pub u32);

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
