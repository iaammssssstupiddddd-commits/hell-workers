//! Water gathering task execution
//!
//! Handles the 5-phase water gathering workflow:
//! 1. GoingToBucket - Navigate to pick up a bucket
//! 2. GoingToRiver - Navigate to water source
//! 3. Filling - Fill the bucket with water
//! 4. GoingToTank - Navigate to storage tank
//! 5. Pouring - Pour water into tank

mod guards;
pub mod helpers;
mod phases;
mod routing;

pub use phases::handle_gather_water_task;
