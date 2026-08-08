mod designation;
mod finalizer;
mod input;
mod native_acceptance;
mod preview;
mod recovery;

pub use designation::deconstruction_designation_system;
#[cfg(feature = "profiling")]
pub(crate) use finalizer::DeconstructionPerfMetrics;
pub use finalizer::deconstruction_finalizer_system;
pub use input::deconstruction_designation_input_system;
pub use native_acceptance::NativeDeconstructionAcceptancePlugin;
pub(crate) use preview::deconstruction_hover_preview_system;
pub(crate) use preview::{DeconstructionHoverPreview, DeconstructionHoverStatus};

use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeconstructionFinalizerSet {
    Flush,
    Finalize,
}

#[cfg(test)]
mod tests;
