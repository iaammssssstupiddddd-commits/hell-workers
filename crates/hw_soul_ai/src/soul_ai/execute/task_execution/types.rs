//! タスク実行関連の型定義

pub use hw_jobs::tasks::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource, BuildData, BuildPhase, CoatWallData, CoatWallPhase, CollectBoneData,
    CollectBonePhase, CollectSandData, CollectSandPhase, FrameWallPhase, FrameWallTileData,
    GatherData, GatherPhase, GeneratePowerData, GeneratePowerPhase, HaulData, HaulPhase,
    HaulToBlueprintData, HaulToBpPhase, HaulToMixerData, HaulToMixerPhase, HaulWithWheelbarrowData,
    HaulWithWheelbarrowPhase, MovePlantData, MovePlantPhase, MovePlantTask, PourFloorPhase,
    PourFloorTileData, RefineData, RefinePhase, ReinforceFloorPhase, ReinforceFloorTileData,
};
