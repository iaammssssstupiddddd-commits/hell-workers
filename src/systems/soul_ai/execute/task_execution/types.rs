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
    /// 建物を移動する
    MovePlant(MovePlantData),
    /// バケツ搬送（川→タンク or タンク→ミキサー）
    BucketTransport(BucketTransportData),
    /// 砂を採取する
    CollectSand(CollectSandData),
    /// 骨を採取する
    CollectBone(CollectBoneData),
    /// 精製作業を行う
    Refine(RefineData),
    /// ミキサーへ資材を運搬する
    HaulToMixer(HaulToMixerData),
    /// 手押し車で一括運搬
    HaulWithWheelbarrow(HaulWithWheelbarrowData),
    /// 床タイルの骨補強
    ReinforceFloorTile(ReinforceFloorTileData),
    /// 床タイルへの泥注入
    PourFloorTile(PourFloorTileData),
    /// 壁タイルの木材フレーミング
    FrameWallTile(FrameWallTileData),
    /// 壁タイルの泥塗布
    CoatWall(CoatWallData),
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct BucketTransportData {
    pub bucket: Entity,
    pub source: BucketTransportSource,
    pub destination: BucketTransportDestination,
    /// Mixer 側へ水を届ける量。現在の運搬では 1 バケツ前提でも、将来の可変量対応のため保持。
    pub amount: u32,
    pub phase: BucketTransportPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub enum BucketTransportSource {
    /// 河川から直接汲む
    River,
    /// タンクを経由して汲む。`needs_fill == true` の場合、タンクへの補充を優先する
    Tank { tank: Entity, needs_fill: bool },
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub enum BucketTransportDestination {
    Tank(Entity),
    Mixer(Entity),
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum BucketTransportPhase {
    #[default]
    GoingToBucket,
    /// source (河川 or タンク) へ移動中
    GoingToSource,
    Filling {
        progress: f32,
    },
    /// destination (タンク or ミキサー) へ移動中
    GoingToDestination,
    Pouring {
        progress: f32,
    },
    ReturningBucket,
}

impl BucketTransportData {
    pub fn source_entity(&self) -> Entity {
        match self.source {
            BucketTransportSource::River => self.bucket,
            BucketTransportSource::Tank { tank, .. } => tank,
        }
    }

    pub fn destination_entity(&self) -> Entity {
        match self.destination {
            BucketTransportDestination::Tank(entity) => entity,
            BucketTransportDestination::Mixer(entity) => entity,
        }
    }

    /// バケツ source（バケツ自体）の予約保持有無を判定する。
    pub fn should_reserve_bucket_source(&self) -> bool {
        matches!(self.phase, BucketTransportPhase::GoingToBucket)
    }

    /// Tank source 予約保持有無を判定する（needs_fill=true 時のみ）。
    /// Phase は旧実装の `GoingToBucket` / `GoingToTank` / `FillingFromTank` 相当。
    pub fn should_reserve_tank_source(&self) -> bool {
        match self.source {
            BucketTransportSource::Tank { needs_fill: true, .. } => matches!(
                self.phase,
                BucketTransportPhase::GoingToBucket
                    | BucketTransportPhase::GoingToSource
                    | BucketTransportPhase::Filling { .. }
            ),
            BucketTransportSource::Tank { .. } | BucketTransportSource::River => false,
        }
    }

    /// Mixer destination 予約保持有無を判定する。
    pub fn should_reserve_mixer_destination(&self) -> bool {
        matches!(self.destination, BucketTransportDestination::Mixer(_))
            && !matches!(self.phase, BucketTransportPhase::ReturningBucket)
    }

    /// Tank source entity を直接参照するための補助 getter。
    pub fn tank_source_entity(&self) -> Option<Entity> {
        match self.source {
            BucketTransportSource::Tank { tank, .. } => Some(tank),
            BucketTransportSource::River => None,
        }
    }
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
    Reinforcing {
        progress_bp: u16,
    },
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
    Pouring {
        progress_bp: u16,
    },
    Done,
}

/// Frame wall tile task data
#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct FrameWallTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: FrameWallPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum FrameWallPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpWood,
    GoingToTile,
    Framing {
        progress_bp: u16,
    },
    Done,
}

/// Coat wall tile task data
#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct CoatWallData {
    pub tile: Entity,
    pub site: Entity,
    pub wall: Entity,
    pub phase: CoatWallPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct MovePlantData {
    pub task_entity: Entity,
    pub building: Entity,
    pub destination_grid: (i32, i32),
    pub destination_pos: Vec2,
    pub companion_anchor: Option<(i32, i32)>,
    pub phase: MovePlantPhase,
}

#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct MovePlantTask {
    pub building: Entity,
    pub destination_grid: (i32, i32),
    pub destination_pos: Vec2,
    pub companion_anchor: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum MovePlantPhase {
    #[default]
    GoToBuilding,
    Moving,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum CoatWallPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpMud,
    GoingToTile,
    Coating {
        progress_bp: u16,
    },
    Done,
}

impl AssignedTask {
    /// Bucket transport データを直接取得する。
    pub fn bucket_transport_data(&self) -> Option<BucketTransportData> {
        match self {
            AssignedTask::BucketTransport(data) => Some(data.clone()),
            _ => None,
        }
    }

    /// タスクの作業タイプを取得
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

    /// タスクのターゲットエンティティを取得（完了イベント用）
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

    /// インベントリ整合性チェック用: タスクが期待するアイテム（バケツ・手押し車等）を返す
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
