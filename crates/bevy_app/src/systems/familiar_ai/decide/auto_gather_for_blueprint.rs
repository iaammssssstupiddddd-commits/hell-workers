//! Blueprint 自動資材収集 facade — 実装は hw_familiar_ai に移設済み。

pub use hw_familiar_ai::familiar_ai::decide::auto_gather_for_blueprint::AutoGatherDesignation;
pub use hw_familiar_ai::familiar_ai::decide::blueprint_auto_gather::{
    BlueprintAutoGatherTimer, blueprint_auto_gather_system,
};
