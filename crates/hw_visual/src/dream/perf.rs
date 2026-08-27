use std::time::Instant;

use bevy::prelude::*;
use hw_core::constants::DREAM_UI_PARTICLE_MAX_ACTIVE;
use hw_core::ui_nodes::UiMountSlot;
use rand::rngs::StdRng;
use rand::{Error, RngCore, SeedableRng};

use super::{
    DreamBubbleUiHandles, DreamGainUiParticle, DreamTrailGhost, spawn_ui_particle_with_rng,
};

const DREAM_UI_PERF_SCHEMA_VERSION: u32 = 2;
const CHECKSUM_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const CHECKSUM_PRIME: u64 = 0x0000_0100_0000_01b3;
const DREAM_UI_PERF_SAMPLE_CAPACITY: usize = 65_536;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DreamUiPerfParticle(pub u32);

#[derive(Resource)]
pub struct DreamUiPerfControl {
    armed: bool,
    seed: u64,
    deterministic_order: bool,
    reset_requested: bool,
    next_ordinal: u32,
    rng: StdRng,
    sequence_checksum: u64,
}

impl DreamUiPerfControl {
    pub fn new(seed: u64, deterministic_order: bool) -> Self {
        Self {
            armed: true,
            seed,
            deterministic_order,
            reset_requested: false,
            next_ordinal: 0,
            rng: StdRng::seed_from_u64(seed),
            sequence_checksum: CHECKSUM_OFFSET,
        }
    }

    pub const fn armed(&self) -> bool {
        self.armed
    }

    pub const fn deterministic_order(&self) -> bool {
        self.deterministic_order
    }

    pub fn reset_measurement(&mut self) {
        self.rng = StdRng::seed_from_u64(self.seed);
        self.next_ordinal = 0;
        self.reset_requested = true;
        self.sequence_checksum = CHECKSUM_OFFSET;
    }

    fn take_reset_request(&mut self) -> bool {
        std::mem::take(&mut self.reset_requested)
    }

    pub const fn sequence_checksum(&self) -> u64 {
        self.sequence_checksum
    }

    fn next_ordinal(&mut self) -> u32 {
        let ordinal = self.next_ordinal;
        self.next_ordinal = self.next_ordinal.wrapping_add(1);
        ordinal
    }
}

impl RngCore for DreamUiPerfControl {
    fn next_u32(&mut self) -> u32 {
        let value = self.rng.next_u32();
        self.sequence_checksum = hash_u64(self.sequence_checksum, u64::from(value));
        value
    }

    fn next_u64(&mut self) -> u64 {
        let value = self.rng.next_u64();
        self.sequence_checksum = hash_u64(self.sequence_checksum, value);
        value
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        self.rng.fill_bytes(destination);
        for chunk in destination.chunks(8) {
            let mut value = [0; 8];
            value[..chunk.len()].copy_from_slice(chunk);
            self.sequence_checksum = hash_u64(self.sequence_checksum, u64::from_le_bytes(value));
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Error> {
        self.rng.try_fill_bytes(destination)?;
        for chunk in destination.chunks(8) {
            let mut value = [0; 8];
            value[..chunk.len()].copy_from_slice(chunk);
            self.sequence_checksum = hash_u64(self.sequence_checksum, u64::from_le_bytes(value));
        }
        Ok(())
    }
}

#[derive(Resource, Debug, Clone)]
pub struct DreamUiPerfMetrics {
    pub schema_version: u32,
    pub measured_frames: u64,
    pub active_particle_updates: u64,
    pub merge_pair_comparisons: u64,
    pub node_writes: u64,
    pub ui_transform_writes: u64,
    pub particle_spawns: u64,
    pub particle_despawns: u64,
    pub trail_spawns: u64,
    pub trail_despawns: u64,
    pub scoped_alloc_calls: u64,
    pub scoped_alloc_bytes: u64,
    pub scoped_allocator_available: bool,
    pub dream_lane_elapsed_ns: u64,
    pub dream_lane_sample_overflow: u64,
    pub rng_sequence_checksum: u64,
    pub trajectory_checksum: u64,
    pub lifetime_checksum: u64,
    pub maximum_active_particles: u32,
    current_frame_elapsed_ns: u64,
    frame_elapsed_samples_ns: Vec<u64>,
}

impl Default for DreamUiPerfMetrics {
    fn default() -> Self {
        Self {
            schema_version: DREAM_UI_PERF_SCHEMA_VERSION,
            measured_frames: 0,
            active_particle_updates: 0,
            merge_pair_comparisons: 0,
            node_writes: 0,
            ui_transform_writes: 0,
            particle_spawns: 0,
            particle_despawns: 0,
            trail_spawns: 0,
            trail_despawns: 0,
            scoped_alloc_calls: 0,
            scoped_alloc_bytes: 0,
            scoped_allocator_available: cfg!(feature = "profiling-memory"),
            dream_lane_elapsed_ns: 0,
            dream_lane_sample_overflow: 0,
            rng_sequence_checksum: CHECKSUM_OFFSET,
            trajectory_checksum: CHECKSUM_OFFSET,
            lifetime_checksum: CHECKSUM_OFFSET,
            maximum_active_particles: 0,
            current_frame_elapsed_ns: 0,
            frame_elapsed_samples_ns: Vec::with_capacity(DREAM_UI_PERF_SAMPLE_CAPACITY),
        }
    }
}

impl DreamUiPerfMetrics {
    pub fn reset_measurement(&mut self) {
        self.schema_version = DREAM_UI_PERF_SCHEMA_VERSION;
        self.measured_frames = 0;
        self.active_particle_updates = 0;
        self.merge_pair_comparisons = 0;
        self.node_writes = 0;
        self.ui_transform_writes = 0;
        self.particle_spawns = 0;
        self.particle_despawns = 0;
        self.trail_spawns = 0;
        self.trail_despawns = 0;
        self.scoped_alloc_calls = 0;
        self.scoped_alloc_bytes = 0;
        self.scoped_allocator_available = cfg!(feature = "profiling-memory");
        self.dream_lane_elapsed_ns = 0;
        self.dream_lane_sample_overflow = 0;
        self.rng_sequence_checksum = CHECKSUM_OFFSET;
        self.trajectory_checksum = CHECKSUM_OFFSET;
        self.lifetime_checksum = CHECKSUM_OFFSET;
        self.maximum_active_particles = 0;
        self.current_frame_elapsed_ns = 0;
        self.frame_elapsed_samples_ns.clear();
    }

    pub fn record_elapsed(&mut self, started: Instant) {
        let elapsed_ns = started.elapsed().as_nanos() as u64;
        self.dream_lane_elapsed_ns = self.dream_lane_elapsed_ns.saturating_add(elapsed_ns);
        self.current_frame_elapsed_ns = self.current_frame_elapsed_ns.saturating_add(elapsed_ns);
    }

    pub fn record_rng_value(&mut self, value: u64) {
        self.rng_sequence_checksum = hash_u64(self.rng_sequence_checksum, value);
    }

    pub fn record_merge_comparison(&mut self) {
        self.merge_pair_comparisons = self.merge_pair_comparisons.saturating_add(1);
    }

    #[cfg(feature = "profiling-memory")]
    pub fn record_scoped_allocations(
        &mut self,
        measurement: hw_core::profiling_alloc_scope::ScopedAllocationMeasurement,
    ) {
        self.scoped_alloc_calls = self.scoped_alloc_calls.saturating_add(measurement.calls);
        self.scoped_alloc_bytes = self.scoped_alloc_bytes.saturating_add(measurement.bytes);
    }

    pub fn finish_frame(&mut self) {
        self.measured_frames = self.measured_frames.saturating_add(1);
        if self.frame_elapsed_samples_ns.len() < DREAM_UI_PERF_SAMPLE_CAPACITY {
            self.frame_elapsed_samples_ns
                .push(self.current_frame_elapsed_ns);
        } else {
            self.dream_lane_sample_overflow = self.dream_lane_sample_overflow.saturating_add(1);
        }
        self.current_frame_elapsed_ns = 0;
    }

    pub fn dream_lane_p95_ns(&self) -> u64 {
        if self.frame_elapsed_samples_ns.is_empty() {
            return 0;
        }
        let mut samples = self.frame_elapsed_samples_ns.clone();
        samples.sort_unstable();
        let rank = samples.len().saturating_mul(95).div_ceil(100).max(1);
        samples[rank - 1]
    }
}

pub fn maintain_dream_ui_perf_burst_system(
    mut commands: Commands,
    mut control: Option<ResMut<DreamUiPerfControl>>,
    handles: Res<DreamBubbleUiHandles>,
    roots: Query<(Entity, &UiMountSlot)>,
    particles: Query<(), With<DreamUiPerfParticle>>,
    particle_entities: Query<Entity, With<DreamUiPerfParticle>>,
    trail_entities: Query<Entity, With<DreamTrailGhost>>,
    #[cfg(feature = "profiling-memory")] mut metrics: Option<ResMut<DreamUiPerfMetrics>>,
) {
    let Some(control) = control.as_deref_mut() else {
        return;
    };
    if !control.armed() {
        return;
    }
    let reset_requested = control.take_reset_request();
    if reset_requested {
        for entity in &particle_entities {
            commands.entity(entity).try_despawn();
        }
        for entity in &trail_entities {
            commands.entity(entity).try_despawn();
        }
    }
    let Some(root) = roots
        .iter()
        .find(|(_, slot)| matches!(slot, UiMountSlot::DreamBubbleLayer))
        .map(|(entity, _)| entity)
    else {
        return;
    };
    #[cfg(feature = "profiling-memory")]
    if metrics.is_some() {
        hw_core::profiling_alloc_scope::begin();
    }
    let active_particles = if reset_requested {
        0
    } else {
        particles.iter().count()
    };
    let missing = DREAM_UI_PARTICLE_MAX_ACTIVE.saturating_sub(active_particles);
    for _ in 0..missing {
        let ordinal = control.next_ordinal();
        let lane = ordinal % DREAM_UI_PARTICLE_MAX_ACTIVE as u32;
        let column = lane % 16;
        let row = lane / 16;
        let start = Vec2::new(96.0 + column as f32 * 34.0, 128.0 + row as f32 * 28.0);
        let target = Vec2::new(1120.0, 64.0);
        let entity =
            spawn_ui_particle_with_rng(&mut commands, start, target, root, &handles, 1.0, control);
        commands.entity(entity).insert(DreamUiPerfParticle(ordinal));
    }
    #[cfg(feature = "profiling-memory")]
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.record_scoped_allocations(hw_core::profiling_alloc_scope::end());
    }
}

pub fn observe_dream_ui_perf_system(
    mut metrics: Option<ResMut<DreamUiPerfMetrics>>,
    control: Option<Res<DreamUiPerfControl>>,
    particles: Query<(&DreamUiPerfParticle, &DreamGainUiParticle, &Node)>,
    added_particles: Query<(), Added<DreamUiPerfParticle>>,
    added_trails: Query<(), Added<DreamTrailGhost>>,
    mut removed_particles: RemovedComponents<DreamUiPerfParticle>,
    mut removed_trails: RemovedComponents<DreamTrailGhost>,
) {
    let Some(metrics) = metrics.as_deref_mut() else {
        return;
    };
    if let Some(control) = control {
        metrics.rng_sequence_checksum = control.sequence_checksum();
    }
    metrics.particle_spawns = metrics
        .particle_spawns
        .saturating_add(added_particles.iter().count() as u64);
    metrics.trail_spawns = metrics
        .trail_spawns
        .saturating_add(added_trails.iter().count() as u64);
    metrics.particle_despawns = metrics
        .particle_despawns
        .saturating_add(removed_particles.read().count() as u64);
    metrics.trail_despawns = metrics
        .trail_despawns
        .saturating_add(removed_trails.read().count() as u64);

    let mut states = particles
        .iter()
        .map(|(marker, particle, node)| {
            (
                marker.0,
                particle.time_alive.to_bits(),
                particle.velocity.x.to_bits(),
                particle.velocity.y.to_bits(),
                px_bits(node.left),
                px_bits(node.top),
            )
        })
        .collect::<Vec<_>>();
    states.sort_unstable_by_key(|state| state.0);
    metrics.maximum_active_particles = metrics
        .maximum_active_particles
        .max(u32::try_from(states.len()).unwrap_or(u32::MAX));
    let mut frame_trajectory = CHECKSUM_OFFSET;
    let mut frame_lifetime = CHECKSUM_OFFSET;
    for (ordinal, lifetime, velocity_x, velocity_y, left, top) in states {
        frame_trajectory = hash_u64(frame_trajectory, u64::from(ordinal));
        frame_trajectory = hash_u64(frame_trajectory, u64::from(velocity_x));
        frame_trajectory = hash_u64(frame_trajectory, u64::from(velocity_y));
        frame_trajectory = hash_u64(frame_trajectory, u64::from(left));
        frame_trajectory = hash_u64(frame_trajectory, u64::from(top));
        frame_lifetime = hash_u64(frame_lifetime, u64::from(ordinal));
        frame_lifetime = hash_u64(frame_lifetime, u64::from(lifetime));
    }
    metrics.trajectory_checksum = hash_u64(metrics.trajectory_checksum, frame_trajectory);
    metrics.lifetime_checksum = hash_u64(metrics.lifetime_checksum, frame_lifetime);
    metrics.finish_frame();
}

fn px_bits(value: Val) -> u32 {
    match value {
        Val::Px(value) => value.to_bits(),
        _ => 0,
    }
}

fn hash_u64(mut checksum: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        checksum = (checksum ^ u64::from(byte)).wrapping_mul(CHECKSUM_PRIME);
    }
    checksum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_restores_schema_and_checksum_seeds() {
        let mut metrics = DreamUiPerfMetrics::default();
        metrics.measured_frames = 9;
        metrics.record_rng_value(7);
        metrics.reset_measurement();

        assert_eq!(metrics.schema_version, DREAM_UI_PERF_SCHEMA_VERSION);
        assert_eq!(metrics.measured_frames, 0);
        assert_eq!(metrics.rng_sequence_checksum, CHECKSUM_OFFSET);
        assert_eq!(metrics.trajectory_checksum, CHECKSUM_OFFSET);
        assert_eq!(metrics.lifetime_checksum, CHECKSUM_OFFSET);
    }

    #[test]
    fn control_reset_restarts_rng_and_requests_fixture_rebuild() {
        let mut control = DreamUiPerfControl::new(42, true);
        let first = control.next_u64();
        let _ = control.next_u64();

        control.reset_measurement();

        assert!(control.take_reset_request());
        assert!(!control.take_reset_request());
        assert_eq!(control.next_ordinal(), 0);
        assert_eq!(control.next_u64(), first);
    }

    #[test]
    fn p95_samples_survive_reset_without_losing_capacity() {
        let mut metrics = DreamUiPerfMetrics::default();
        let initial_capacity = metrics.frame_elapsed_samples_ns.capacity();
        for elapsed_ns in 1..=100 {
            metrics.current_frame_elapsed_ns = elapsed_ns;
            metrics.finish_frame();
        }

        assert_eq!(metrics.dream_lane_p95_ns(), 95);
        metrics.reset_measurement();

        assert_eq!(metrics.dream_lane_p95_ns(), 0);
        assert_eq!(
            metrics.frame_elapsed_samples_ns.capacity(),
            initial_capacity
        );
        assert_eq!(metrics.dream_lane_sample_overflow, 0);
    }
}
