//! タスク実行関連の型定義

pub mod bucket;
pub mod build;
pub mod collect;
pub mod gather;
pub mod haul;
pub mod move_plant;
pub mod refine;
pub mod wheelbarrow;

pub use bucket::{
    BucketTransportData, BucketTransportDestination, BucketTransportPhase, BucketTransportSource,
};
pub use build::{
    BuildData, BuildPhase, CoatWallData, CoatWallPhase, FrameWallPhase, FrameWallTileData,
    PourFloorPhase, PourFloorTileData, ReinforceFloorPhase, ReinforceFloorTileData,
};
pub use collect::{CollectBoneData, CollectBonePhase, CollectSandData, CollectSandPhase};
pub use gather::{GatherData, GatherPhase};
pub use haul::{HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase};
pub use move_plant::{MovePlantData, MovePlantPhase, MovePlantTask};
pub use refine::{HaulToMixerData, HaulToMixerPhase, RefineData, RefinePhase};
pub use wheelbarrow::{HaulWithWheelbarrowData, HaulWithWheelbarrowPhase};

use bevy::prelude::*;
use hw_core::jobs::WorkType;

#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub enum AssignedTask {
    #[default]
    None,
    Gather(GatherData),
    Haul(HaulData),
    HaulToBlueprint(HaulToBlueprintData),
    Build(BuildData),
    MovePlant(MovePlantData),
    BucketTransport(BucketTransportData),
    CollectSand(CollectSandData),
    CollectBone(CollectBoneData),
    Refine(RefineData),
    HaulToMixer(HaulToMixerData),
    HaulWithWheelbarrow(HaulWithWheelbarrowData),
    ReinforceFloorTile(ReinforceFloorTileData),
    PourFloorTile(PourFloorTileData),
    FrameWallTile(FrameWallTileData),
    CoatWall(CoatWallData),
}

impl AssignedTask {
    pub fn bucket_transport_data(&self) -> Option<BucketTransportData> {
        match self {
            AssignedTask::BucketTransport(data) => Some(data.clone()),
            _ => None,
        }
    }

    pub fn work_type(&self) -> Option<WorkType> {
        match self {
            AssignedTask::Gather(data) => Some(data.work_type),
            AssignedTask::Haul(_) => Some(WorkType::Haul),
            AssignedTask::HaulToBlueprint(_) => Some(WorkType::Haul),
            AssignedTask::Build(_) => Some(WorkType::Build),
            AssignedTask::MovePlant(_) => Some(WorkType::Move),
            AssignedTask::BucketTransport(data) => match data.source {
                BucketTransportSource::River => Some(WorkType::GatherWater),
                BucketTransportSource::Tank { .. } => Some(WorkType::HaulWaterToMixer),
            },
            AssignedTask::CollectSand(_) => Some(WorkType::CollectSand),
            AssignedTask::CollectBone(_) => Some(WorkType::CollectBone),
            AssignedTask::Refine(_) => Some(WorkType::Refine),
            AssignedTask::HaulToMixer(_) => Some(WorkType::Haul),
            AssignedTask::HaulWithWheelbarrow(_) => Some(WorkType::WheelbarrowHaul),
            AssignedTask::ReinforceFloorTile(_) => Some(WorkType::ReinforceFloorTile),
            AssignedTask::PourFloorTile(_) => Some(WorkType::PourFloorTile),
            AssignedTask::FrameWallTile(_) => Some(WorkType::FrameWallTile),
            AssignedTask::CoatWall(_) => Some(WorkType::CoatWall),
            AssignedTask::None => None,
        }
    }

    pub fn get_target_entity(&self) -> Option<Entity> {
        match self {
            AssignedTask::Gather(data) => Some(data.target),
            AssignedTask::Haul(data) => Some(data.item),
            AssignedTask::HaulToBlueprint(data) => Some(data.item),
            AssignedTask::Build(data) => Some(data.blueprint),
            AssignedTask::MovePlant(data) => Some(data.building),
            AssignedTask::BucketTransport(data) => Some(data.bucket),
            AssignedTask::CollectSand(data) => Some(data.target),
            AssignedTask::CollectBone(data) => Some(data.target),
            AssignedTask::Refine(data) => Some(data.mixer),
            AssignedTask::HaulToMixer(data) => Some(data.item),
            AssignedTask::HaulWithWheelbarrow(data) => Some(data.wheelbarrow),
            AssignedTask::ReinforceFloorTile(data) => Some(data.tile),
            AssignedTask::PourFloorTile(data) => Some(data.tile),
            AssignedTask::FrameWallTile(data) => Some(data.tile),
            AssignedTask::CoatWall(data) => Some(data.tile),
            AssignedTask::None => None,
        }
    }

    pub fn get_amount_if_haul_water(&self) -> Option<u32> {
        if let AssignedTask::BucketTransport(data) = self {
            Some(data.amount)
        } else {
            None
        }
    }

    pub fn expected_item(&self) -> Option<Entity> {
        match self {
            AssignedTask::Haul(data) => Some(data.item),
            AssignedTask::HaulToBlueprint(data) => Some(data.item),
            AssignedTask::HaulToMixer(data) => Some(data.item),
            AssignedTask::BucketTransport(data) => Some(data.bucket),
            AssignedTask::HaulWithWheelbarrow(data) => Some(data.wheelbarrow),
            _ => None,
        }
    }

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
            AssignedTask::BucketTransport(data) => {
                !matches!(data.phase, BucketTransportPhase::GoingToBucket)
            }
            AssignedTask::HaulWithWheelbarrow(data) => !matches!(
                data.phase,
                HaulWithWheelbarrowPhase::GoingToParking
                    | HaulWithWheelbarrowPhase::PickingUpWheelbarrow
            ),
            _ => false,
        }
    }
}
