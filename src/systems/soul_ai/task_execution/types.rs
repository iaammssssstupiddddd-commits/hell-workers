//! タスク実行関連の型定義

use crate::systems::jobs::WorkType;
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
    Collecting { progress: f32 },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum BuildPhase {
    #[default]
    GoingToBlueprint,
    Building { progress: f32 },
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
    Filling { progress: f32 },
    GoingToTank,
    Pouring { progress: f32 },
    ReturningBucket,
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
            AssignedTask::None => None,
        }
    }
}
