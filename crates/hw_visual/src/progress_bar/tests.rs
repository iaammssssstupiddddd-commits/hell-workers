use super::*;
use crate::{floor_construction, soul, wall_construction};
use hw_core::soul::{DamnedSoul, SoulUiLinks};
use hw_core::visual_mirror::{
    construction::{FloorConstructionPhaseMirror, FloorSiteVisualState, WallSiteVisualState},
    task::SoulTaskVisualState,
};

#[test]
fn progress_pair_owns_siblings_and_keeps_left_edge_on_parent_motion_test() {
    let mut app = App::new();
    app.add_plugins(TransformPlugin);
    let owner = app
        .world_mut()
        .spawn(Transform::from_xyz(100.0, 200.0, 3.0))
        .id();
    let config = ProgressBarConfig::default();
    let pair = spawn_progress_bar(&mut app.world_mut().commands(), owner, config.clone());
    app.world_mut().flush();
    for entity in [pair.background, pair.fill] {
        assert_eq!(app.world().get::<ChildOf>(entity).unwrap().parent(), owner);
    }
    for progress in [0.0, 0.5, 1.0] {
        let mut sprite = app.world().get::<Sprite>(pair.fill).unwrap().clone();
        let mut transform = *app.world().get::<Transform>(pair.fill).unwrap();
        update_progress_bar_fill(progress, &config, &mut sprite, &mut transform, None);
        let width = sprite.custom_size.unwrap().x;
        sync_progress_bar_fill_position(&Transform::default(), &config, width, &mut transform);
        assert_eq!(transform.translation.x - width / 2.0, -config.width / 2.0);
        app.world_mut()
            .entity_mut(pair.fill)
            .insert((sprite, transform));
        app.world_mut()
            .get_mut::<Transform>(owner)
            .unwrap()
            .translation
            .x += 10.0;
        app.update();
        let parent_pos = app
            .world()
            .get::<GlobalTransform>(owner)
            .unwrap()
            .translation();
        let fill_pos = app
            .world()
            .get::<GlobalTransform>(pair.fill)
            .unwrap()
            .translation();
        assert_eq!(fill_pos.x - width / 2.0, parent_pos.x - config.width / 2.0);
        assert_eq!(fill_pos.y, parent_pos.y + config.y_offset);
        assert_eq!(
            fill_pos.z,
            parent_pos.z + Z_BAR_FILL - Z_BAR_BG + config.z_index
        );
    }
    app.world_mut().entity_mut(owner).despawn();
    assert!(app.world().get_entity(pair.background).is_err());
    assert!(app.world().get_entity(pair.fill).is_err());
}

fn bar_children(world: &World, owner: Entity) -> Vec<Entity> {
    world
        .get::<Children>(owner)
        .map(|children| {
            children
                .iter()
                .filter(|child| world.get::<GenericProgressBar>(*child).is_some())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn progress_sites_reconcile_phase_end_and_owner_removal_test() {
    let mut app = App::new();
    app.add_systems(
        Update,
        (
            floor_construction::manage_floor_curing_progress_bars_system,
            wall_construction::manage_wall_progress_bars_system,
        ),
    );
    let floor = app
        .world_mut()
        .spawn((
            Transform::default(),
            FloorSiteVisualState {
                phase: FloorConstructionPhaseMirror::Curing,
                curing_remaining_secs: 5.0,
                tiles_total: 1,
            },
        ))
        .id();
    let wall = app
        .world_mut()
        .spawn((
            Transform::default(),
            WallSiteVisualState {
                phase_is_framing: true,
                tiles_total: 2,
                tiles_framed: 1,
                tiles_coated: 0,
            },
        ))
        .id();
    app.update();
    let floor_bars = bar_children(app.world(), floor);
    let wall_bars = bar_children(app.world(), wall);
    assert_eq!(floor_bars.len(), 2);
    assert_eq!(wall_bars.len(), 2);
    app.update();
    assert_eq!(bar_children(app.world(), floor), floor_bars);
    assert_eq!(bar_children(app.world(), wall), wall_bars);
    app.world_mut()
        .get_mut::<FloorSiteVisualState>(floor)
        .unwrap()
        .curing_remaining_secs = 0.0;
    app.world_mut()
        .get_mut::<WallSiteVisualState>(wall)
        .unwrap()
        .tiles_framed = 2;
    app.update();
    for bar in floor_bars.into_iter().chain(wall_bars) {
        assert!(app.world().get_entity(bar).is_err());
    }
    app.world_mut()
        .get_mut::<WallSiteVisualState>(wall)
        .unwrap()
        .phase_is_framing = false;
    app.update();
    let bars = bar_children(app.world(), wall);
    assert_eq!(bars.len(), 2);
    app.world_mut().entity_mut(wall).despawn();
    for bar in bars {
        assert!(app.world().get_entity(bar).is_err());
    }
}

#[test]
fn soul_progress_none_removes_both_children_and_link_test() {
    let mut app = App::new();
    app.add_systems(Update, soul::progress_bar_system);
    let owner = app
        .world_mut()
        .spawn((
            DamnedSoul::default(),
            Transform::default(),
            SoulUiLinks::default(),
            SoulTaskVisualState {
                progress: Some(0.5),
                ..default()
            },
        ))
        .id();
    app.update();
    let bars = bar_children(app.world(), owner);
    assert_eq!(bars.len(), 2);
    assert!(
        app.world()
            .get::<SoulUiLinks>(owner)
            .unwrap()
            .bar_entity
            .is_some()
    );
    app.world_mut()
        .get_mut::<SoulTaskVisualState>(owner)
        .unwrap()
        .progress = None;
    app.update();
    assert!(
        app.world()
            .get::<SoulUiLinks>(owner)
            .unwrap()
            .bar_entity
            .is_none()
    );
    for bar in bars {
        assert!(app.world().get_entity(bar).is_err());
    }
}
