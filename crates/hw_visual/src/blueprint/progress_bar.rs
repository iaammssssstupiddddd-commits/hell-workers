//! 設計図のプログレスバー関連システム

use bevy::prelude::ChildOf;
use bevy::prelude::*;
use hw_core::constants::Z_BAR_BG;

use super::components::{BlueprintProgressBars, ProgressBar};
use super::{
    COLOR_PROGRESS_BG, COLOR_PROGRESS_BUILD, COLOR_PROGRESS_MATERIAL, PROGRESS_BAR_HEIGHT,
    PROGRESS_BAR_WIDTH, PROGRESS_BAR_Y_OFFSET,
};
use crate::progress_bar::{
    GenericProgressBar, ProgressBarConfig, ProgressBarFill, spawn_progress_bar,
    update_progress_bar_fill,
};
use hw_core::visual_mirror::construction::BlueprintVisualState;

type BlueprintWithoutBarsQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform),
    (With<BlueprintVisualState>, Without<BlueprintProgressBars>),
>;

type BlueprintProgressUpdateQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static BlueprintVisualState,
        &'static BlueprintProgressBars,
    ),
    Or<(Changed<BlueprintVisualState>, Added<BlueprintProgressBars>)>,
>;

type ProgressFillQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Sprite, &'static mut Transform),
    (With<ProgressBar>, With<ProgressBarFill>),
>;

pub fn spawn_progress_bar_system(mut commands: Commands, q_blueprints: BlueprintWithoutBarsQuery) {
    for (bp_entity, bp_transform) in q_blueprints.iter() {
        let config = ProgressBarConfig {
            width: PROGRESS_BAR_WIDTH,
            height: PROGRESS_BAR_HEIGHT,
            y_offset: PROGRESS_BAR_Y_OFFSET,
            bg_color: COLOR_PROGRESS_BG,
            fill_color: COLOR_PROGRESS_MATERIAL,
            z_index: Z_BAR_BG,
        };

        let (bg_entity, fill_entity) =
            spawn_progress_bar(&mut commands, bp_entity, bp_transform, config);

        commands.entity(bg_entity).insert(ProgressBar);
        commands.entity(fill_entity).insert(ProgressBar);

        commands.entity(bg_entity).try_insert(ChildOf(bp_entity));
        commands.entity(fill_entity).try_insert(ChildOf(bp_entity));
        commands.entity(bp_entity).insert(BlueprintProgressBars {
            background: bg_entity,
            fill: fill_entity,
        });
    }
}

pub fn update_progress_bar_fill_system(
    q_blueprints: BlueprintProgressUpdateQuery,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_fills: ProgressFillQuery,
) {
    for (state, bars) in q_blueprints.iter() {
        let Ok(generic_bar) = q_generic_bars.get(bars.fill) else {
            continue;
        };
        let Ok((mut sprite, mut transform)) = q_fills.get_mut(bars.fill) else {
            continue;
        };

        let total_required: u32 = state.material_counts.iter().map(|(_, _, r)| r).sum::<u32>()
            + state
                .flexible_material
                .as_ref()
                .map(|(_, _, r)| *r)
                .unwrap_or(0);
        let total_delivered: u32 = state.material_counts.iter().map(|(_, d, _)| d).sum::<u32>()
            + state
                .flexible_material
                .as_ref()
                .map(|(_, d, _)| *d)
                .unwrap_or(0);

        let material_ratio = if total_required > 0 {
            (total_delivered as f32 / total_required as f32).min(1.0)
        } else {
            1.0
        };

        let combined_progress = material_ratio * 0.5 + state.progress.min(1.0) * 0.5;

        let fill_color = if state.progress > 0.0 {
            Some(COLOR_PROGRESS_BUILD)
        } else {
            Some(COLOR_PROGRESS_MATERIAL)
        };

        update_progress_bar_fill(
            combined_progress,
            &generic_bar.config,
            &mut sprite,
            &mut transform,
            fill_color,
        );
    }
}

pub fn cleanup_progress_bars_system(
    mut commands: Commands,
    mut removed_blueprints: RemovedComponents<BlueprintVisualState>,
    q_blueprints: Query<(), With<BlueprintVisualState>>,
    q_progress_bars: Query<&BlueprintProgressBars>,
) {
    for blueprint in removed_blueprints.read() {
        if q_blueprints.get(blueprint).is_ok() {
            continue;
        }
        let Ok(bars) = q_progress_bars.get(blueprint) else {
            continue;
        };

        commands.entity(bars.background).try_despawn();
        commands.entity(bars.fill).try_despawn();
        commands.entity(blueprint).remove::<BlueprintProgressBars>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                spawn_progress_bar_system,
                update_progress_bar_fill_system,
                cleanup_progress_bars_system,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn progress_bars_are_owned_by_the_blueprint_and_removed_with_visual_state() {
        let mut app = test_app();
        let blueprint = app
            .world_mut()
            .spawn((BlueprintVisualState::default(), Transform::default()))
            .id();

        app.update();
        app.update();

        let bars = *app
            .world()
            .get::<BlueprintProgressBars>(blueprint)
            .expect("blueprint progress bars should be linked to their owner");
        assert!(app.world().get_entity(bars.background).is_ok());
        assert!(app.world().get_entity(bars.fill).is_ok());
        let progress_bar_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<ProgressBar>>();
            query.iter(world).count()
        };
        assert_eq!(progress_bar_count, 2);

        app.world_mut()
            .entity_mut(blueprint)
            .remove::<BlueprintVisualState>();
        app.update();

        assert!(
            app.world()
                .get::<BlueprintProgressBars>(blueprint)
                .is_none()
        );
        assert!(app.world().get_entity(bars.background).is_err());
        assert!(app.world().get_entity(bars.fill).is_err());
    }

    #[test]
    fn unchanged_blueprints_do_not_rewrite_progress_bar_transforms() {
        let mut app = test_app();
        let blueprint = app
            .world_mut()
            .spawn((BlueprintVisualState::default(), Transform::default()))
            .id();

        app.update();
        app.update();
        let bars = *app
            .world()
            .get::<BlueprintProgressBars>(blueprint)
            .expect("blueprint progress bars should be linked to their owner");

        app.world_mut().clear_trackers();
        app.update();

        let changed_bar_transforms = {
            let world = app.world_mut();
            let mut query =
                world.query_filtered::<Entity, (With<ProgressBar>, Changed<Transform>)>();
            query.iter(world).count()
        };
        assert_eq!(changed_bar_transforms, 0);
        assert!(app.world().get_entity(bars.fill).is_ok());
    }

    #[test]
    fn changed_visual_state_updates_the_linked_fill() {
        let mut app = test_app();
        let blueprint = app
            .world_mut()
            .spawn((BlueprintVisualState::default(), Transform::default()))
            .id();

        app.update();
        app.update();
        let bars = *app
            .world()
            .get::<BlueprintProgressBars>(blueprint)
            .expect("blueprint progress bars should be linked to their owner");

        app.world_mut()
            .get_mut::<BlueprintVisualState>(blueprint)
            .expect("blueprint should retain its visual state")
            .progress = 0.5;
        app.update();

        let sprite = app
            .world()
            .get::<Sprite>(bars.fill)
            .expect("progress bar fill should exist");
        assert_eq!(sprite.custom_size, Some(Vec2::new(18.0, 3.0)));
        let transform = app
            .world()
            .get::<Transform>(bars.fill)
            .expect("progress bar fill should have a transform");
        assert_eq!(transform.translation.x, -3.0);
    }

    #[test]
    fn despawning_the_blueprint_despawns_its_progress_bar_children() {
        let mut app = test_app();
        let blueprint = app
            .world_mut()
            .spawn((BlueprintVisualState::default(), Transform::default()))
            .id();

        app.update();
        let bars = *app
            .world()
            .get::<BlueprintProgressBars>(blueprint)
            .expect("blueprint progress bars should be linked to their owner");

        app.world_mut().entity_mut(blueprint).despawn();

        assert!(app.world().get_entity(bars.background).is_err());
        assert!(app.world().get_entity(bars.fill).is_err());
    }
}
