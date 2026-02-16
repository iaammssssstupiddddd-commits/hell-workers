//! タスクハンドラのトレイトとディスパッチ

mod dispatch;
mod impls;
mod task_handler;

pub use dispatch::{execute_haul_with_wheelbarrow, run_task_handler};
pub use task_handler::TaskHandler;
