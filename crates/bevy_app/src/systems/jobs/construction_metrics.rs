//! Profiling-only work counters for construction transitions.
//!
//! The counters deliberately describe work performed rather than elapsed time:
//! frame time stays comparable across hosts, while these values make it clear
//! whether an incomplete site, a tile index, or a curing safety audit caused a
//! scan during the measurement window.

#[cfg(feature = "profiling")]
use bevy::prelude::*;

#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default)]
pub struct ConstructionPerfMetrics {
    pub floor_sites_considered: u64,
    pub wall_sites_considered: u64,
    pub floor_tiles_inspected: u64,
    pub wall_tiles_inspected: u64,
    pub evacuation_candidates_scanned: u64,
    pub floor_phase_elapsed_micros: u64,
    pub floor_completion_elapsed_micros: u64,
    pub wall_phase_elapsed_micros: u64,
    pub wall_completion_elapsed_micros: u64,
}
