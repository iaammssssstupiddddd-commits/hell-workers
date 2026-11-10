use super::*;
use crate::list::models::{EntityListSnapshot, SoulGender, StressBucket, TaskVisual};
use crate::list::values::tests::TestAssets;
use std::{cell::Cell, sync::Once};

thread_local! { static WARNINGS:Cell<usize> = const {Cell::new(0)}; }
struct Probe;
impl log::Log for Probe {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            WARNINGS.with(|n| n.set(n.get() + 1));
        }
    }
    fn flush(&self) {}
}
fn start_probe() {
    static INIT: Once = Once::new();
    static LOGGER: Probe = Probe;
    INIT.call_once(|| {
        log::set_logger(&LOGGER).expect("test logger");
        log::set_max_level(log::LevelFilter::Warn);
    });
    WARNINGS.with(|n| n.set(0));
}
fn assert_clean() {
    assert_eq!(WARNINGS.with(Cell::get), 0, "real Bevy command WARN/ERROR");
}

#[derive(Resource)]
struct Containers {
    familiars: Entity,
    unassigned: Entity,
}

#[derive(bevy::ecs::system::SystemParam)]
struct UiQueries<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    text: Query<'w, 's, &'static mut Text>,
    images: Query<'w, 's, &'static mut ImageNode>,
}

fn sync(
    mut commands: Commands,
    assets: Res<TestAssets>,
    theme: Res<UiTheme>,
    containers: Res<Containers>,
    mut vm: ResMut<EntityListViewModel>,
    mut index: ResMut<EntityListNodeIndex>,
    mut queries: UiQueries,
) {
    sync_familiar_sections(
        &mut commands,
        &*assets,
        &theme,
        &mut FamiliarSectionCtx {
            view_model: &vm,
            node_index: &mut index,
            fam_container_entity: containers.familiars,
        },
        &queries.children,
        &mut queries.text,
        &mut queries.images,
    );
    sync_unassigned_souls(
        &mut commands,
        &*assets,
        &theme,
        &vm,
        &mut index,
        containers.unassigned,
        &queries.children,
    );
    vm.previous = vm.current.clone();
}

fn soul(entity: Entity) -> SoulRowViewModel {
    SoulRowViewModel {
        entity,
        name: "Brenda".into(),
        gender: SoulGender::Female,
        fatigue_text: "82%".into(),
        stress_text: "35%".into(),
        stress_bucket: StressBucket::Low,
        dream_text: "58".into(),
        dream_empty: false,
        task_visual: TaskVisual::Build,
    }
}
fn fixture() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.init_resource::<EntityListViewModel>()
        .init_resource::<EntityListNodeIndex>()
        .init_resource::<TestAssets>()
        .init_resource::<UiTheme>()
        .add_systems(Update, sync);
    let fam_container = app.world_mut().spawn(Node::default()).id();
    let unassigned = app.world_mut().spawn(Node::default()).id();
    app.insert_resource(Containers {
        familiars: fam_container,
        unassigned,
    });
    let fam = app.world_mut().spawn_empty().id();
    let other = app.world_mut().spawn_empty().id();
    let s = app.world_mut().spawn_empty().id();
    let other_s = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<EntityListViewModel>()
        .current = EntityListSnapshot {
        familiars: vec![
            FamiliarRowViewModel {
                entity: fam,
                label: "one".into(),
                is_folded: false,
                show_empty: false,
                souls: vec![soul(s)],
            },
            FamiliarRowViewModel {
                entity: other,
                label: "two".into(),
                is_folded: false,
                show_empty: false,
                souls: vec![soul(other_s)],
            },
        ],
        ..Default::default()
    };
    start_probe();
    app.update();
    assert_clean();
    (app, fam, other)
}
fn children(world: &World, parent: Entity) -> Vec<Entity> {
    world
        .get::<Children>(parent)
        .map(|c| c.iter().collect())
        .unwrap_or_default()
}

#[test]
fn member_row_to_empty_has_one_deletion_owner_test() {
    let (mut app, fam, other) = fixture();
    let index = app.world().resource::<EntityListNodeIndex>();
    let container = index.familiar_sections[&fam].members_container;
    let old = children(app.world(), container)[0];
    let old_children = children(app.world(), old);
    let other_row = children(
        app.world(),
        index.familiar_sections[&other].members_container,
    )[0];
    app.world_mut()
        .resource_mut::<EntityListViewModel>()
        .current
        .familiars[0]
        .souls
        .clear();
    start_probe();
    app.update();
    assert_clean();
    assert!(children(app.world(), container).is_empty());
    for e in std::iter::once(old).chain(old_children) {
        assert!(app.world().get_entity(e).is_err());
    }
    assert!(app.world().get_entity(container).is_ok());
    assert!(app.world().get_entity(other_row).is_ok());
}

#[test]
fn all_sections_to_empty_have_one_deletion_owner_test() {
    let (mut app, _, _) = fixture();
    let container = app.world().resource::<Containers>().familiars;
    let roots = children(app.world(), container);
    app.world_mut()
        .resource_mut::<EntityListViewModel>()
        .current
        .familiars
        .clear();
    start_probe();
    app.update();
    assert_clean();
    assert!(children(app.world(), container).is_empty());
    assert!(
        app.world()
            .resource::<EntityListNodeIndex>()
            .familiar_sections
            .is_empty()
    );
    for root in roots {
        assert!(app.world().get_entity(root).is_err());
    }
}

#[test]
fn empty_hint_fold_search_zero_and_recovery_keep_owned_rows_test() {
    let (mut app, fam, other) = fixture();
    let index = app.world().resource::<EntityListNodeIndex>();
    let container = index.familiar_sections[&fam].members_container;
    let other_row = children(
        app.world(),
        index.familiar_sections[&other].members_container,
    )[0];
    let original = app
        .world()
        .resource::<EntityListViewModel>()
        .current
        .familiars[0]
        .souls
        .clone();
    for (folded, hint, present) in [
        (false, true, false),
        (false, false, false),
        (true, false, false),
        (false, false, false),
        (false, false, true),
        (false, false, false),
        (false, false, true),
    ] {
        {
            let mut vm = app.world_mut().resource_mut::<EntityListViewModel>();
            let f = &mut vm.current.familiars[0];
            f.is_folded = folded;
            f.show_empty = hint;
            f.souls = if present { original.clone() } else { vec![] };
        }
        start_probe();
        app.update();
        assert_clean();
        assert_eq!(
            children(app.world(), container).len(),
            usize::from(!folded && (hint || present))
        );
        assert!(app.world().get_entity(other_row).is_ok());
    }
}
