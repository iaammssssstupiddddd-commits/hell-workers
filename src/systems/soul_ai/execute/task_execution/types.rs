//! タスク実行関連の型定義

use crate::systems::jobs::WorkType;
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use bevy::prelude::*;

/// 魂に割り当てられたタスク
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub enum AssignedTask {
    #[default]
    None,
    /// リソースを収集する
    Gather(GatherData),
    /// リソースを運搬する（ストックパイルへ）
    Haul(HaulData),
    /// 資材を設計図へ運搬する
    HaulToBlueprint(HaulToBlueprintData),
    /// 建築作業を行う
    Build(BuildData),
    /// 水汲みを行う
    GatherWater(GatherWaterData),
    /// 砂を採取する
    CollectSand(CollectSandData),
    /// 骨を採取する
    CollectBone(CollectBoneData),
    /// 精製作業を行う
    Refine(RefineData),
    /// ミキサーへ資材を運搬する
    HaulToMixer(HaulToMixerData),
    /// Tankの水をバケツでMudMixerへ運ぶ
    HaulWaterToMixer(HaulWaterToMixerData),
    /// 手押し車で一括運搬
    HaulWithWheelbarrow(HaulWithWheelbarrowData),
    /// 床タイルの骨補強
    ReinforceFloorTile(ReinforceFloorTileData),
    /// 床タイルへの泥注入
    PourFloorTile(PourFloorTileData),
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct GatherData {
    pub target: Entity,
    pub work_type: WorkType,
    pub phase: GatherPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulData {
    pub item: Entity,
    pub stockpile: Entity,
    pub phase: HaulPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulToBlueprintData {
    pub item: Entity,
    pub blueprint: Entity,
    pub phase: HaulToBpPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct BuildData {
    pub blueprint: Entity,
    pub phase: BuildPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct GatherWaterData {
    pub bucket: Entity,
    pub tank: Entity,
    pub phase: GatherWaterPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct CollectSandData {
    pub target: Entity,
    pub phase: CollectSandPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct CollectBoneData {
    pub target: Entity,
    pub phase: CollectBonePhase,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct RefineData {
    pub mixer: Entity,
    pub phase: RefinePhase,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulToMixerData {
    pub item: Entity,
    pub mixer: Entity,
    pub resource_type: crate::systems::logistics::ResourceType,
    pub phase: HaulToMixerPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulWaterToMixerData {
    pub bucket: Entity,
    pub tank: Entity,
    pub mixer: Entity,
    pub amount: u32,
    pub phase: HaulWaterToMixerPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulPhase {
    #[default]
    GoingToItem,
    GoingToStockpile,
    Dropping,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum GatherPhase {
    #[default]
    GoingToResource,
    Collecting {
        progress: f32,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum BuildPhase {
    #[default]
    GoingToBlueprint,
    Building {
        progress: f32,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulToBpPhase {
    #[default]
    GoingToItem,
    GoingToBlueprint,
    Delivering,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum GatherWaterPhase {
    #[default]
    GoingToBucket,
    GoingToRiver,
    Filling {
        progress: f32,
    },
    GoingToTank,
    Pouring {
        progress: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum CollectSandPhase {
    #[default]
    GoingToSand,
    Collecting {
        progress: f32,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum CollectBonePhase {
    #[default]
    GoingToBone,
    Collecting {
        progress: f32,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum RefinePhase {
    #[default]
    GoingToMixer,
    Refining {
        progress: f32,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulToMixerPhase {
    #[default]
    GoingToItem,
    GoingToMixer,
    Delivering,
}

/// 手押し車による一括運搬タスク
#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct HaulWithWheelbarrowData {
    pub wheelbarrow: Entity,
    pub source_pos: Vec2,
    pub destination: WheelbarrowDestination,
    /// 直接採取モードの採取元（通常運搬では None）
    pub collect_source: Option<Entity>,
    /// 直接採取モードの採取量（通常運搬では 0）
    pub collect_amount: u32,
    /// 直接採取モードで生成する資源種別（通常運搬では None）
    pub collect_resource_type: Option<crate::systems::logistics::ResourceType>,
    pub items: Vec<Entity>,
    pub phase: HaulWithWheelbarrowPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulWithWheelbarrowPhase {
    #[default]
    GoingToParking,
    PickingUpWheelbarrow,
    GoingToSource,
    Loading,
    GoingToDestination,
    Unloading,
    ReturningWheelbarrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulWaterToMixerPhase {
    #[default]
    GoingToBucket,
    GoingToTank,
    FillingFromTank,
    GoingToMixer,
    Pouring,
    ReturningBucket,
}

/// Reinforce floor tile task data
#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct ReinforceFloorTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: ReinforceFloorPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum ReinforceFloorPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpBones,
    GoingToTile,
    Reinforcing { progress: u8 },
    Done,
}

/// Pour floor tile task data
#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct PourFloorTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: PourFloorPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum PourFloorPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpMud,
    GoingToTile,
    Pouring { progress: u8 },
    Done,
}

impl AssignedTask {
    /// タスクの作業タイプを取得
    pub fn work_type(&self) -> Option<WorkType> {
        match self {
            AssignedTask::Gather(data) => Some(data.work_type),
            AssignedTask::Haul(_) => Some(WorkType::Haul),
            AssignedTask::HaulToBlueprint(_) => Some(WorkType::Haul),
            AssignedTask::Build(_) => Some(WorkType::Build),
            AssignedTask::GatherWater(_) => Some(WorkType::GatherWater),
            AssignedTask::CollectSand(_) => Some(WorkType::CollectSand),
            AssignedTask::CollectBone(_) => Some(WorkType::CollectBone),
            AssignedTask::Refine(_) => Some(WorkType::Refine),
            AssignedTask::HaulToMixer(_) => Some(WorkType::Haul),
            AssignedTask::HaulWaterToMixer(_) => Some(WorkType::HaulWaterToMixer),
            AssignedTask::HaulWithWheelbarrow(_) => Some(WorkType::WheelbarrowHaul),
            AssignedTask::ReinforceFloorTile(_) => Some(WorkType::ReinforceFloorTile),
            AssignedTask::PourFloorTile(_) => Some(WorkType::PourFloorTile),
            AssignedTask::None => None,
        }
    }

    /// タスクのターゲットエンティティを取得（完了イベント用）
    pub fn get_target_entity(&self) -> Option<Entity> {
        match self {
            AssignedTask::Gather(data) => Some(data.target),
            AssignedTask::Haul(data) => Some(data.item),
            AssignedTask::HaulToBlueprint(data) => Some(data.item),
            AssignedTask::Build(data) => Some(data.blueprint),
            AssignedTask::GatherWater(data) => Some(data.bucket),
            AssignedTask::CollectSand(data) => Some(data.target),
            AssignedTask::CollectBone(data) => Some(data.target),
            AssignedTask::Refine(data) => Some(data.mixer),
            AssignedTask::HaulToMixer(data) => Some(data.item),
            AssignedTask::HaulWaterToMixer(data) => Some(data.bucket),
            AssignedTask::HaulWithWheelbarrow(data) => Some(data.wheelbarrow),
            AssignedTask::ReinforceFloorTile(data) => Some(data.tile),
            AssignedTask::PourFloorTile(data) => Some(data.tile),
            AssignedTask::None => None,
        }
    }

    pub fn get_amount_if_haul_water(&self) -> Option<u32> {
        if let AssignedTask::HaulWaterToMixer(data) = self {
            Some(data.amount)
        } else {
            None
        }
    }

    /// インベントリ整合性チェック用: タスクが期待するアイテム（バケツ・手押し車等）を返す
    pub fn expected_item(&self) -> Option<Entity> {
        match self {
            AssignedTask::Haul(data) => Some(data.item),
            AssignedTask::HaulToBlueprint(data) => Some(data.item),
            AssignedTask::HaulToMixer(data) => Some(data.item),
            AssignedTask::GatherWater(data) => Some(data.bucket),
            AssignedTask::HaulWaterToMixer(data) => Some(data.bucket),
            AssignedTask::HaulWithWheelbarrow(data) => Some(data.wheelbarrow),
            _ => None,
        }
    }

    /// インベントリ整合性チェック用: 現在フェーズでインベントリにアイテムが必須か
    pub fn requires_item_in_inventory(&self) -> bool {
        match self {
            AssignedTask::Haul(data) => matches!(data.phase, HaulPhase::GoingToStockpile),
            AssignedTask::HaulToBlueprint(data) => {
                matches!(data.phase, HaulToBpPhase::GoingToBlueprint)
            }
            AssignedTask::HaulToMixer(data) => matches!(
                data.phase,
                HaulToMixerPhase::GoingToMixer | HaulToMixerPhase::Delivering
            ),
            AssignedTask::GatherWater(data) => {
                !matches!(data.phase, GatherWaterPhase::GoingToBucket)
            }
            AssignedTask::HaulWaterToMixer(data) => {
                !matches!(data.phase, HaulWaterToMixerPhase::GoingToBucket)
            }
            AssignedTask::HaulWithWheelbarrow(data) => {
                !matches!(
                    data.phase,
                    HaulWithWheelbarrowPhase::GoingToParking
                        | HaulWithWheelbarrowPhase::PickingUpWheelbarrow
                )
            }
            _ => false,
        }
    }
}
