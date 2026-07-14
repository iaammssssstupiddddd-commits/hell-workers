//! Shared ECS helpers with lifecycle-reader contracts.

use bevy::prelude::{Component, Entity, RemovedComponents};

/// Consume every pending removal and report whether at least one was observed.
///
/// `RemovedComponents::read()` only advances its cursor through the portion of
/// the iterator that is consumed. Use this instead of taking a single removal
/// when the entity ids are not needed.
pub fn drain_removed<T: Component>(removed: &mut RemovedComponents<T>) -> bool {
    let mut any_removed = false;
    for _ in removed.read() {
        any_removed = true;
    }
    any_removed
}

/// Consume every pending removal while returning whether any entity matched.
///
/// The predicate intentionally runs for every removal. This avoids leaving
/// unread lifecycle messages behind when an early entity matches.
pub fn drain_removed_where<T: Component>(
    removed: &mut RemovedComponents<T>,
    mut predicate: impl FnMut(Entity) -> bool,
) -> bool {
    let mut matched = false;
    for entity in removed.read() {
        matched |= predicate(entity);
    }
    matched
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::prelude::*;

    #[derive(Component)]
    struct First;

    #[derive(Component)]
    struct Second;

    #[derive(Resource, Default)]
    struct DrainReport {
        non_empty_reads: u32,
        unread_after_drain: usize,
    }

    fn consume_first(mut removed: RemovedComponents<First>, mut report: ResMut<DrainReport>) {
        if drain_removed(&mut removed) {
            report.non_empty_reads += 1;
        }
        report.unread_after_drain += removed.read().count();
    }

    #[derive(Resource)]
    struct PredicateReport {
        expected: Entity,
        matched: bool,
        seen: Vec<Entity>,
    }

    fn consume_first_where(
        mut removed: RemovedComponents<First>,
        mut report: ResMut<PredicateReport>,
    ) {
        let expected = report.expected;
        let mut seen = Vec::new();
        let matched = drain_removed_where(&mut removed, |entity| {
            seen.push(entity);
            entity == expected
        });

        report.matched |= matched;
        report.seen.extend(seen);
    }

    #[derive(Resource, Default)]
    struct MultipleReaderReport {
        first_seen: bool,
        second_seen: bool,
    }

    fn consume_multiple_readers(
        mut first: RemovedComponents<First>,
        mut second: RemovedComponents<Second>,
        mut report: ResMut<MultipleReaderReport>,
    ) {
        report.first_seen |= drain_removed(&mut first);
        report.second_seen |= drain_removed(&mut second);
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app
    }

    #[test]
    fn drain_removed_consumes_every_pending_entity() {
        let mut app = test_app();
        app.init_resource::<DrainReport>();
        app.add_systems(Update, consume_first);
        app.update();

        let first = app.world_mut().spawn(First).id();
        let second = app.world_mut().spawn(First).id();
        app.world_mut().entity_mut(first).remove::<First>();
        app.world_mut().entity_mut(second).remove::<First>();

        app.update();
        let report = app.world().resource::<DrainReport>();
        assert_eq!(report.non_empty_reads, 1);
        assert_eq!(report.unread_after_drain, 0);

        app.update();
        let report = app.world().resource::<DrainReport>();
        assert_eq!(report.non_empty_reads, 1);
        assert_eq!(report.unread_after_drain, 0);
    }

    #[test]
    fn drain_removed_where_evaluates_every_pending_entity() {
        let mut app = test_app();
        let expected = app.world_mut().spawn(First).id();
        let other = app.world_mut().spawn(First).id();
        app.insert_resource(PredicateReport {
            expected,
            matched: false,
            seen: Vec::new(),
        });
        app.add_systems(Update, consume_first_where);
        app.update();

        app.world_mut().entity_mut(expected).remove::<First>();
        app.world_mut().entity_mut(other).remove::<First>();
        app.update();

        let report = app.world().resource::<PredicateReport>();
        assert!(report.matched);
        assert!(report.seen.contains(&expected));
        assert!(report.seen.contains(&other));
    }

    #[test]
    fn separate_readers_consume_removals_in_the_same_update() {
        let mut app = test_app();
        app.init_resource::<MultipleReaderReport>();
        app.add_systems(Update, consume_multiple_readers);
        app.update();

        let first = app.world_mut().spawn(First).id();
        let second = app.world_mut().spawn(Second).id();
        app.world_mut().entity_mut(first).remove::<First>();
        app.world_mut().entity_mut(second).remove::<Second>();
        app.update();

        let report = app.world().resource::<MultipleReaderReport>();
        assert!(report.first_seen);
        assert!(report.second_seen);
    }
}
