//! タスク実行関連の型定義

pub mod bucket;
pub mod build;
pub mod collect;
pub mod gather;
pub mod generate_power;
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
pub use collect::{CollectBoneData, CollectBonePhase};
pub use gather::{GatherData, GatherPhase};
pub use generate_power::{GeneratePowerData, GeneratePowerPhase};
pub use haul::{HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase};
pub use move_plant::{MovePlantData, MovePlantPhase, MovePlantTask};
pub use refine::{HaulToMixerData, HaulToMixerPhase, RefineData, RefinePhase};
pub use wheelbarrow::{HaulWithWheelbarrowData, HaulWithWheelbarrowPhase};

use bevy::prelude::*;
use hw_core::jobs::WorkType;

/// 実行中 assignment の相関情報。
///
/// assignment の起点と、chain 後の現在 segment を分けて保持する。runtime-only の
/// 状態であり、セーブ対象には含めない。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveTaskIdentity {
    pub assignment_entity: Entity,
    pub current_target_entity: Entity,
    pub current_work_type: WorkType,
    binding: TaskIdentityBinding,
}

/// `ActiveTaskIdentity` と `WorkingOn` の対応状態。
///
/// 通常の実行 segment は必ず `Attached` である。Gather / Refine が成果物を確定して
/// 次 frame の Done phase を待つ間だけ `Detached` を使うため、外部要因による
/// `WorkingOn` の消失を正常な状態として扱わない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskIdentityBinding {
    Attached,
    Detached,
}

impl ActiveTaskIdentity {
    pub fn new(
        assignment_entity: Entity,
        current_target_entity: Entity,
        current_work_type: WorkType,
    ) -> Self {
        Self {
            assignment_entity,
            current_target_entity,
            current_work_type,
            binding: TaskIdentityBinding::Attached,
        }
    }

    /// 同じ assignment 内で次の task segment へ移行する。
    pub fn transition_to(&mut self, current_target_entity: Entity, current_work_type: WorkType) {
        self.current_target_entity = current_target_entity;
        self.current_work_type = current_work_type;
        self.binding = TaskIdentityBinding::Attached;
    }

    /// 成果確定後、次 frame の Done phase まで `WorkingOn` を持たないことを明示する。
    pub fn detach_from_working_on(&mut self) {
        self.binding = TaskIdentityBinding::Detached;
    }

    /// 現在の `WorkingOn` と identity の組が実行可能な状態か判定する。
    pub fn matches_working_on(&self, working_on: Option<Entity>) -> bool {
        match self.binding {
            TaskIdentityBinding::Attached => working_on == Some(self.current_target_entity),
            TaskIdentityBinding::Detached => working_on.is_none(),
        }
    }
}

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
    CollectBone(CollectBoneData),
    Refine(RefineData),
    HaulToMixer(HaulToMixerData),
    HaulWithWheelbarrow(HaulWithWheelbarrowData),
    ReinforceFloorTile(ReinforceFloorTileData),
    PourFloorTile(PourFloorTileData),
    FrameWallTile(FrameWallTileData),
    CoatWall(CoatWallData),
    GeneratePower(GeneratePowerData),
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
            AssignedTask::CollectBone(_) => Some(WorkType::CollectBone),
            AssignedTask::Refine(_) => Some(WorkType::Refine),
            AssignedTask::HaulToMixer(_) => Some(WorkType::HaulToMixer),
            AssignedTask::HaulWithWheelbarrow(_) => Some(WorkType::WheelbarrowHaul),
            AssignedTask::ReinforceFloorTile(_) => Some(WorkType::ReinforceFloorTile),
            AssignedTask::PourFloorTile(_) => Some(WorkType::PourFloorTile),
            AssignedTask::FrameWallTile(_) => Some(WorkType::FrameWallTile),
            AssignedTask::CoatWall(_) => Some(WorkType::CoatWall),
            AssignedTask::GeneratePower(_) => Some(WorkType::GeneratePower),
            AssignedTask::None => None,
        }
    }

    pub fn primary_payload_entity(&self) -> Option<Entity> {
        match self {
            AssignedTask::Gather(data) => Some(data.target),
            AssignedTask::Haul(data) => Some(data.item),
            AssignedTask::HaulToBlueprint(data) => Some(data.item),
            AssignedTask::Build(data) => Some(data.blueprint),
            AssignedTask::MovePlant(data) => Some(data.building),
            AssignedTask::BucketTransport(data) => Some(data.bucket),
            AssignedTask::CollectBone(data) => Some(data.target),
            AssignedTask::Refine(data) => Some(data.mixer),
            AssignedTask::HaulToMixer(data) => Some(data.item),
            AssignedTask::HaulWithWheelbarrow(data) => Some(data.wheelbarrow),
            AssignedTask::ReinforceFloorTile(data) => Some(data.tile),
            AssignedTask::PourFloorTile(data) => Some(data.tile),
            AssignedTask::FrameWallTile(data) => Some(data.tile),
            AssignedTask::CoatWall(data) => Some(data.tile),
            AssignedTask::GeneratePower(data) => Some(data.tile),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_task_identity_transition_preserves_assignment() {
        let mut world = World::new();
        let assignment = world.spawn_empty().id();
        let initial_target = world.spawn_empty().id();
        let next_target = world.spawn_empty().id();
        let mut identity = ActiveTaskIdentity::new(assignment, initial_target, WorkType::Chop);

        identity.transition_to(next_target, WorkType::HaulToMixer);

        assert_eq!(identity.assignment_entity, assignment);
        assert_eq!(identity.current_target_entity, next_target);
        assert_eq!(identity.current_work_type, WorkType::HaulToMixer);
        assert!(identity.matches_working_on(Some(next_target)));
    }

    #[test]
    fn detached_identity_only_allows_an_absent_working_on() {
        let mut world = World::new();
        let assignment = world.spawn_empty().id();
        let target = world.spawn_empty().id();
        let mut identity = ActiveTaskIdentity::new(assignment, target, WorkType::Chop);

        assert!(!identity.matches_working_on(None));
        identity.detach_from_working_on();
        assert!(identity.matches_working_on(None));
        assert!(!identity.matches_working_on(Some(target)));
    }

    #[test]
    fn haul_to_mixer_reports_its_distinct_work_type() {
        let task = AssignedTask::HaulToMixer(HaulToMixerData {
            item: Entity::PLACEHOLDER,
            mixer: Entity::PLACEHOLDER,
            resource_type: hw_core::logistics::ResourceType::Rock,
            phase: HaulToMixerPhase::GoingToItem,
        });

        assert_eq!(task.work_type(), Some(WorkType::HaulToMixer));
    }
}
