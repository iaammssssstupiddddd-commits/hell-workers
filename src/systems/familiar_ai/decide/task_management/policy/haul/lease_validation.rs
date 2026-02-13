//! WheelbarrowLease の有効性検証

use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::logistics::transport_request::WheelbarrowLease;

/// WheelbarrowLease の有効性を検証
///
/// - wheelbarrow がまだ利用可能（parked かつ未使用）か
/// - items のうち最低 `min_valid_items` 個が未予約の地面アイテムか
pub fn validate_lease(
    lease: &WheelbarrowLease,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
    min_valid_items: usize,
) -> bool {
    use crate::systems::familiar_ai::decide::task_management::validator::source_not_reserved;

    if queries.wheelbarrows.get(lease.wheelbarrow).is_err() {
        return false;
    }
    if !source_not_reserved(lease.wheelbarrow, queries, shadow) {
        return false;
    }
    let valid_count = lease
        .items
        .iter()
        .filter(|item| source_not_reserved(**item, queries, shadow))
        .count();
    valid_count >= min_valid_items
}
