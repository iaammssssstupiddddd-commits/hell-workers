//! Building move system (root shell)
//!
//! Root shell: `TransportRequest` / `AssignedTask` / `unassign_task` + `WorldMap` 占有更新に依存。
//! CompanionPlacement + task unassign を扱うため hw_ui / hw_jobs への移設不可。
//! 純バリデーション API は hw_ui::selection::placement の WorldReadApi を参照。
pub(crate) mod placement;
mod preview;
mod system;

pub use preview::building_move_preview_system;
pub use system::building_move_system;
