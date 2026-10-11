//! Optional profiling receipts emitted only from the actual placement branch.
//! These never drive input, create owners, or grant lifecycle/release authority.
use bevy::prelude::*;
use bevy::time::Real;
use serde_json::{Value, json};

#[derive(Resource, Default)]
pub(crate) struct PlacementReceipts {
    pub rows: Vec<Value>,
    pub failed: bool,
}

impl PlacementReceipts {
    fn push(&mut self, row: Value) {
        if self.rows.len() >= 4096 {
            self.failed = true;
        } else {
            self.rows.push(row);
        }
    }
}

pub(crate) fn record(commands: &mut Commands, outcome: Value) {
    commands.queue(move |world: &mut World| {
        if !world.contains_resource::<PlacementReceipts>() {
            return;
        }
        let mut windows =
            world.query_filtered::<Entity, (With<bevy::window::PrimaryWindow>, With<Window>)>();
        let window = windows.single(world).ok();
        let time = world
            .get_resource::<Time<Real>>()
            .map(|time| time.elapsed_secs_f64());
        let epoch = world
            .get_resource::<hw_core::WorldEpoch>()
            .map(|epoch| epoch.get());
        let mut receipts = world.resource_mut::<PlacementReceipts>();
        match (window, time, epoch) {
            (Some(window), Some(time), Some(epoch)) if time.is_finite() => {
                receipts.push(json!({"producer": "ordinary-building-placement-branch-v1",
                    "operation": "PlacementOutcome", "real_seconds": time,
                    "window": format!("{window:?}"), "world_epoch": epoch, "outcome": outcome}));
            }
            _ => receipts.failed = true,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(world: &mut World, outcome: Value) {
        let mut queue = bevy::ecs::world::CommandQueue::default();
        record(&mut Commands::new(&mut queue, world), outcome);
        queue.apply(world);
    }

    #[test]
    fn unconfigured_observer_does_not_create_receipt_resources() {
        let mut world = World::new();
        apply(&mut world, json!({"status": "CompanionPending"}));
        assert!(!world.contains_resource::<PlacementReceipts>());
    }

    #[test]
    fn missing_or_ambiguous_window_fails_closed() {
        for window_count in [0, 2] {
            let mut world = World::new();
            world.init_resource::<PlacementReceipts>();
            world.init_resource::<Time<Real>>();
            world.init_resource::<hw_core::WorldEpoch>();
            for _ in 0..window_count {
                world.spawn((Window::default(), bevy::window::PrimaryWindow));
            }
            apply(&mut world, json!({"status": "CompanionPending"}));
            let receipts = world.resource::<PlacementReceipts>();
            assert!(receipts.failed);
            assert!(receipts.rows.is_empty());
        }
    }

    #[test]
    fn pending_receipt_preserves_branch_result_and_exact_context() {
        let mut world = World::new();
        world.init_resource::<PlacementReceipts>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<hw_core::WorldEpoch>();
        let window = world
            .spawn((Window::default(), bevy::window::PrimaryWindow))
            .id();
        let outcome = json!({"kind": "Tank", "grid": [3, 4], "status": "CompanionPending"});
        apply(&mut world, outcome.clone());
        let receipts = world.resource::<PlacementReceipts>();
        assert!(!receipts.failed);
        assert_eq!(receipts.rows.len(), 1);
        assert_eq!(receipts.rows[0]["outcome"], outcome);
        assert_eq!(receipts.rows[0]["window"], format!("{window:?}"));
        assert_eq!(receipts.rows[0]["world_epoch"], 0);
        assert!(receipts.rows[0]["outcome"].get("owner").is_none());
    }

    #[test]
    fn placement_receipts_overflow_fails_without_eviction() {
        let mut receipts = PlacementReceipts::default();
        for index in 0..4097 {
            receipts.push(json!({"index": index}));
        }
        assert!(receipts.failed);
        assert_eq!(receipts.rows.len(), 4096);
        assert_eq!(receipts.rows[0]["index"], 0);
        assert_eq!(receipts.rows[4095]["index"], 4095);
    }
}
