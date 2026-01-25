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
    Gather {
        target: Entity,
        work_type: WorkType,
        phase: GatherPhase,
    },
    /// リソースを運搬する（ストックパイルへ）
    Haul {
        item: Entity,
        stockpile: Entity,
        phase: HaulPhase,
    },
    /// 資材を設計図へ運搬する
    HaulToBlueprint {
        item: Entity,
        blueprint: Entity,
        phase: HaulToBpPhase,
    },
    /// 建築作業を行う
    Build {
        blueprint: Entity,
        phase: BuildPhase,
    },
    /// 水汲みを行う
    GatherWater {
        bucket: Entity,
        tank: Entity,
        phase: GatherWaterPhase,
    },
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
}

impl AssignedTask {
    /// タスクの作業タイプを取得
    pub fn work_type(&self) -> Option<WorkType> {
        match self {
            AssignedTask::Gather { work_type, .. } => Some(*work_type),
            AssignedTask::Haul { .. } => Some(WorkType::Haul),
            AssignedTask::HaulToBlueprint { .. } => Some(WorkType::Haul),
            AssignedTask::Build { .. } => Some(WorkType::Build),
            AssignedTask::GatherWater { .. } => Some(WorkType::GatherWater),
            AssignedTask::None => None,
        }
    }

    /// タスクのターゲットエンティティを取得（完了イベント用）
    pub fn get_target_entity(&self) -> Option<Entity> {
        match self {
            AssignedTask::Gather { target, .. } => Some(*target),
            AssignedTask::Haul { item, .. } => Some(*item),
            AssignedTask::HaulToBlueprint { item, .. } => Some(*item),
            AssignedTask::Build { blueprint, .. } => Some(*blueprint),
            AssignedTask::GatherWater { bucket, .. } => Some(*bucket),
            AssignedTask::None => None,
        }
    }
}
