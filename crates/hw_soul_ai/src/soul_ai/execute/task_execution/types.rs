//! タスク実行関連の型定義

pub use hw_jobs::assigned_task::{
    AssignedTask,
    BucketTransportData, BucketTransportSource, BucketTransportDestination, BucketTransportPhase,
    GatherData, GatherPhase,
    HaulData, HaulPhase,
    HaulToBlueprintData, HaulToBpPhase,
    BuildData, BuildPhase,
    CollectSandData, CollectSandPhase,
    CollectBoneData, CollectBonePhase,
    RefineData, RefinePhase,
    HaulToMixerData, HaulToMixerPhase,
    HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
    ReinforceFloorTileData, ReinforceFloorPhase,
    PourFloorTileData, PourFloorPhase,
    FrameWallTileData, FrameWallPhase,
    CoatWallData, CoatWallPhase,
    MovePlantData, MovePlantTask, MovePlantPhase,
};
