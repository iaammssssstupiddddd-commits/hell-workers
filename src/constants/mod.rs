//! 定数のドメイン別分割
//!
//! 既存の `use crate::constants::*` 互換を維持するため、
//! 全定数を再 export している。

mod ai;
mod animation;
mod conversation;
mod dream;
mod logistics;
mod render;
mod speech;
mod world;

pub use ai::*;
pub use animation::*;
pub use conversation::*;
pub use dream::*;
pub use logistics::*;
pub use render::*;
pub use speech::*;
pub use world::*;
