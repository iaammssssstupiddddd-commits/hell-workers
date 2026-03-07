//! Provisional wall destination demand helpers.

use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};

/// 仮設壁への mud 1個分の基礎需要（incoming/nearby補正前）を返す。
///
/// 返り値は以下を差し引く前の値:
/// - IncomingDeliveries / ReservationShadow
/// - 近傍地面資材数 (`count_nearby_ground_resources`)
/// - リソース別の地面資材在庫補正
pub fn provisional_wall_mud_demand(
    building: &Building,
    provisional_opt: Option<&ProvisionalWall>,
) -> usize {
    if building.kind != BuildingType::Wall || !building.is_provisional {
        return 0;
    }
    if provisional_opt.is_some_and(|provisional| provisional.mud_delivered) {
        return 0;
    }

    1usize
}
