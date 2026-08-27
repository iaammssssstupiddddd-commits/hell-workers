//! Incremental aggregate of Soul refine-task activity by MudMixer.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::WorldEpoch;

use crate::{AssignedTask, RefinePhase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RefineContribution {
    mixer: Entity,
    refining: bool,
}

impl RefineContribution {
    fn from_task(task: &AssignedTask) -> Option<Self> {
        let AssignedTask::Refine(data) = task else {
            return None;
        };
        Some(Self {
            mixer: data.mixer,
            refining: matches!(data.phase, RefinePhase::Refining { .. }),
        })
    }
}

/// Runtime-only counts shared by refine designation and visual projection.
#[derive(Resource, Default)]
pub struct RefineActivityIndex {
    contributions: HashMap<Entity, RefineContribution>,
    assigned_counts: HashMap<Entity, usize>,
    refining_counts: HashMap<Entity, usize>,
    visual_dirty_mixers: HashSet<Entity>,
    epoch: u64,
    initialized: bool,
}

impl RefineActivityIndex {
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    #[must_use]
    pub fn assigned_count(&self, mixer: Entity) -> usize {
        self.assigned_counts.get(&mixer).copied().unwrap_or(0)
    }

    #[must_use]
    pub fn refining_count(&self, mixer: Entity) -> usize {
        self.refining_counts.get(&mixer).copied().unwrap_or(0)
    }

    pub fn mark_visual_dirty(&mut self, mixer: Entity) {
        self.visual_dirty_mixers.insert(mixer);
    }

    pub fn drain_visual_dirty_into(&mut self, out: &mut Vec<Entity>) {
        out.clear();
        out.extend(self.visual_dirty_mixers.drain());
    }

    fn clear_for_epoch(&mut self, epoch: u64) {
        self.contributions.clear();
        self.assigned_counts.clear();
        self.refining_counts.clear();
        self.visual_dirty_mixers.clear();
        self.epoch = epoch;
        self.initialized = false;
    }

    fn update(&mut self, soul: Entity, next: Option<RefineContribution>) {
        let previous = self.contributions.get(&soul).copied();
        if previous == next {
            return;
        }
        if let Some(previous) = previous {
            self.visual_dirty_mixers.insert(previous.mixer);
            decrement(&mut self.assigned_counts, previous.mixer);
            if previous.refining {
                decrement(&mut self.refining_counts, previous.mixer);
            }
            self.contributions.remove(&soul);
        }
        if let Some(next) = next {
            self.visual_dirty_mixers.insert(next.mixer);
            *self.assigned_counts.entry(next.mixer).or_insert(0) += 1;
            if next.refining {
                *self.refining_counts.entry(next.mixer).or_insert(0) += 1;
            }
            self.contributions.insert(soul, next);
        }
    }
}

fn decrement(counts: &mut HashMap<Entity, usize>, mixer: Entity) {
    let Some(count) = counts.get_mut(&mixer) else {
        return;
    };
    *count = count.saturating_sub(1);
    if *count == 0 {
        counts.remove(&mixer);
    }
}

/// Synchronizes semantic refine contributions after Soul task execution.
///
/// Progress-only changes compare equal and do not perturb the aggregate. A
/// world replacement forces one full rebuild even if the numeric epoch of a
/// domain-specific revision happens to repeat.
pub fn sync_refine_activity_index_system(
    world_epoch: Option<Res<WorldEpoch>>,
    q_all_tasks: Query<(Entity, &AssignedTask)>,
    q_changed_tasks: Query<(Entity, &AssignedTask), Changed<AssignedTask>>,
    mut removed_tasks: RemovedComponents<AssignedTask>,
    mut index: ResMut<RefineActivityIndex>,
) {
    let epoch = world_epoch.map_or_else(WorldEpoch::default, |epoch| *epoch);
    if index.epoch != epoch.get() {
        index.clear_for_epoch(epoch.get());
    }

    if !index.initialized {
        index.initialized = true;
        for (entity, task) in &q_all_tasks {
            index.update(entity, RefineContribution::from_task(task));
        }
        return;
    }

    for entity in removed_tasks.read() {
        index.update(entity, None);
    }
    for (entity, task) in &q_changed_tasks {
        index.update(entity, RefineContribution::from_task(task));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RefineData;
    use hw_core::visual_mirror::building::MudMixerVisualState;

    fn refine(mixer: Entity, phase: RefinePhase) -> AssignedTask {
        AssignedTask::Refine(RefineData { mixer, phase })
    }

    #[test]
    fn index_tracks_all_assigned_phases_and_only_refining_visual_phase() {
        let mut app = App::new();
        app.init_resource::<WorldEpoch>()
            .init_resource::<RefineActivityIndex>()
            .add_systems(Update, sync_refine_activity_index_system);
        let mixer = app.world_mut().spawn_empty().id();
        let going = app
            .world_mut()
            .spawn(refine(mixer, RefinePhase::GoingToMixer))
            .id();
        let refining = app
            .world_mut()
            .spawn(refine(mixer, RefinePhase::Refining { progress: 0.25 }))
            .id();
        let done = app.world_mut().spawn(refine(mixer, RefinePhase::Done)).id();

        app.update();
        let index = app.world().resource::<RefineActivityIndex>();
        assert_eq!(index.assigned_count(mixer), 3);
        assert_eq!(index.refining_count(mixer), 1);

        let mut visual_dirty = Vec::new();
        app.world_mut()
            .resource_mut::<RefineActivityIndex>()
            .drain_visual_dirty_into(&mut visual_dirty);
        assert_eq!(visual_dirty, vec![mixer]);

        *app.world_mut()
            .entity_mut(refining)
            .get_mut::<AssignedTask>()
            .unwrap() = refine(mixer, RefinePhase::Refining { progress: 0.75 });
        app.update();
        app.world_mut()
            .resource_mut::<RefineActivityIndex>()
            .drain_visual_dirty_into(&mut visual_dirty);
        assert!(visual_dirty.is_empty());

        *app.world_mut()
            .entity_mut(going)
            .get_mut::<AssignedTask>()
            .unwrap() = AssignedTask::None;
        app.world_mut().despawn(done);
        app.update();

        let index = app.world().resource::<RefineActivityIndex>();
        assert_eq!(index.assigned_count(mixer), 1);
        assert_eq!(index.refining_count(mixer), 1);
    }

    #[test]
    fn epoch_change_rebuilds_from_live_tasks() {
        let mut app = App::new();
        app.init_resource::<WorldEpoch>()
            .init_resource::<RefineActivityIndex>()
            .add_systems(Update, sync_refine_activity_index_system);
        let first_mixer = app.world_mut().spawn_empty().id();
        let soul = app
            .world_mut()
            .spawn(refine(first_mixer, RefinePhase::Done))
            .id();
        app.update();

        let second_mixer = app.world_mut().spawn_empty().id();
        *app.world_mut()
            .entity_mut(soul)
            .get_mut::<AssignedTask>()
            .unwrap() = refine(second_mixer, RefinePhase::GoingToMixer);
        app.world_mut().resource_mut::<WorldEpoch>().advance();
        app.update();

        let index = app.world().resource::<RefineActivityIndex>();
        assert_eq!(index.assigned_count(first_mixer), 0);
        assert_eq!(index.assigned_count(second_mixer), 1);
    }

    #[test]
    fn visual_projection_is_active_only_during_refining() {
        let mut app = App::new();
        app.init_resource::<WorldEpoch>()
            .init_resource::<RefineActivityIndex>()
            .add_systems(
                Update,
                (
                    sync_refine_activity_index_system,
                    crate::visual_sync::sync_mud_mixer_active_system,
                )
                    .chain(),
            );
        let mixer = app.world_mut().spawn(MudMixerVisualState::default()).id();
        let soul = app
            .world_mut()
            .spawn(refine(mixer, RefinePhase::Refining { progress: 0.0 }))
            .id();

        app.update();
        assert!(
            app.world()
                .entity(mixer)
                .get::<MudMixerVisualState>()
                .unwrap()
                .is_active
        );

        *app.world_mut()
            .entity_mut(soul)
            .get_mut::<AssignedTask>()
            .unwrap() = refine(mixer, RefinePhase::Done);
        app.update();
        assert!(
            !app.world()
                .entity(mixer)
                .get::<MudMixerVisualState>()
                .unwrap()
                .is_active
        );
    }
}
