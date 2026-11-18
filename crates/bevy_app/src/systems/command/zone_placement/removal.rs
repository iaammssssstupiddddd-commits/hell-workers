use crate::app_contexts::TaskContext;
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskMode;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::WorldEpoch;
use hw_core::game_state::{PlayMode, TaskModeZoneType};
use hw_logistics::{
    BelongsTo, BucketStorage, PendingBelongsToBlueprint, Stockpile, StockpilePolicy,
};
use hw_ui::camera::MainCamera;
use hw_world::zones::{AreaBounds, Yard};
use hw_world::{StockpileRemovalCell, identify_removal_targets};

use super::removal_preview::{
    RemovalPreview, ZoneRemovalPreviewState, clear_removal_preview, update_removal_preview,
};

type RegularStockpileFilters = (
    With<Stockpile>,
    With<StockpilePolicy>,
    Without<BucketStorage>,
    Without<PendingBelongsToBlueprint>,
);

#[derive(SystemParam)]
pub struct ZoneRemovalInput<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<'w, UiInputState>,
    time: Res<'w, Time<Virtual>>,
    epoch: Res<'w, WorldEpoch>,
    stockpiles: Query<'w, 's, &'static BelongsTo, RegularStockpileFilters>,
    yards: Query<'w, 's, (), With<Yard>>,
}

pub fn zone_removal_system(
    input: ZoneRemovalInput,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
    mut q_sprites: Query<&mut Sprite>,
    mut preview_state: ResMut<ZoneRemovalPreviewState>,
) {
    let TaskMode::ZoneRemoval(kind, mut start) = task_context.0 else {
        if preview_state.is_active() {
            clear_removal_preview(&mut q_sprites, &mut preview_state);
        }
        return;
    };

    let can_use_world = !input.time.is_paused() && !input.ui_input_state.world_input_blocked();
    let snapped = can_use_world
        .then(|| super::world_cursor_pos(&input.q_window, &input.q_camera))
        .flatten()
        .map(WorldMap::snap_to_grid_edge);

    if input.buttons.just_pressed(MouseButton::Right) && can_use_world {
        clear_removal_preview(&mut q_sprites, &mut preview_state);
        if start.is_some() {
            task_context.0 = TaskMode::ZoneRemoval(kind, None);
        } else {
            task_context.0 = TaskMode::None;
            next_play_mode.set(PlayMode::Normal);
        }
        return;
    }

    if input.buttons.just_released(MouseButton::Left) {
        if let Some(preview) = start
            .zip(snapped)
            .map(|(start, end)| plan_removal(&input, world_map.as_ref(), kind, start, end))
            && preview_state.displayed.as_ref() == Some(&preview)
        {
            for cell in preview.targets.cells() {
                if world_map.clear_stockpile_tile_if_owned(cell.grid, cell.entity) {
                    commands.entity(cell.entity).despawn();
                }
            }
        }
        task_context.0 = TaskMode::ZoneRemoval(kind, None);
        clear_removal_preview(&mut q_sprites, &mut preview_state);
        return;
    }

    if !input.buttons.pressed(MouseButton::Left) {
        if start.is_some() {
            task_context.0 = TaskMode::ZoneRemoval(kind, None);
        }
        if preview_state.is_active() {
            clear_removal_preview(&mut q_sprites, &mut preview_state);
        }
        return;
    }

    if input.buttons.just_pressed(MouseButton::Left) && snapped.is_some() {
        start = snapped;
        task_context.0 = TaskMode::ZoneRemoval(kind, start);
    }

    if let Some((start, end)) = start.zip(snapped) {
        let preview = plan_removal(&input, world_map.as_ref(), kind, start, end);
        update_removal_preview(preview, &mut q_sprites, &mut preview_state);
    } else if preview_state.is_active() {
        clear_removal_preview(&mut q_sprites, &mut preview_state);
    }
}

fn plan_removal(
    input: &ZoneRemovalInput,
    world_map: &WorldMap,
    kind: TaskModeZoneType,
    start: Vec2,
    end: Vec2,
) -> RemovalPreview {
    let area = AreaBounds::from_points(start, end);
    let cells = world_map
        .stockpile_entries()
        .filter_map(|(&grid, &entity)| {
            let yard = input.stockpiles.get(entity).ok()?.0;
            input.yards.get(yard).ok()?;
            Some(StockpileRemovalCell { grid, entity, yard })
        });
    RemovalPreview {
        epoch: *input.epoch,
        kind,
        start,
        targets: identify_removal_targets(cells, &area),
        area,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
    use bevy::window::WindowResolution;
    use hw_core::constants::TILE_SIZE;

    fn regular_cell(world: &mut World, grid: (i32, i32), yard: Entity) -> Entity {
        let entity = world
            .spawn((
                Stockpile {
                    capacity: 4,
                    resource_type: None,
                },
                StockpilePolicy::for_capacity(4),
                BelongsTo(yard),
                Sprite::from_color(Color::WHITE, Vec2::splat(TILE_SIZE)),
            ))
            .id();
        world
            .resource_mut::<WorldMap>()
            .register_stockpile_tile(grid, entity);
        entity
    }

    #[test]
    fn release_removes_only_the_displayed_regular_stockpile_cell_test() {
        #[derive(PartialEq, Eq)]
        enum Release {
            World,
            ReplacedCell,
            Hud,
            Paused,
        }
        for release in [
            Release::World,
            Release::ReplacedCell,
            Release::Hud,
            Release::Paused,
        ] {
            let mut app = App::new();
            app.init_resource::<WorldMap>()
                .init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<Time<Virtual>>()
                .init_resource::<WorldEpoch>()
                .init_resource::<NextState<PlayMode>>()
                .init_resource::<ZoneRemovalPreviewState>()
                .init_resource::<UiInputState>()
                .insert_resource(TaskContext(TaskMode::ZoneRemoval(
                    TaskModeZoneType::Stockpile,
                    None,
                )))
                .add_systems(Update, zone_removal_system);
            let yard = app
                .world_mut()
                .spawn(Yard {
                    min: Vec2::ZERO,
                    max: Vec2::ONE,
                })
                .id();
            let targets: Vec<_> = [(10, 10), (11, 10), (12, 10), (13, 10), (20, 20), (21, 20)]
                .into_iter()
                .map(|grid| (grid, regular_cell(app.world_mut(), grid, yard)))
                .collect();
            let equipment = app.world_mut().spawn_empty().id();
            let companions: Vec<_> = [(30, 30), (31, 30)]
                .into_iter()
                .map(|grid| {
                    let entity = app
                        .world_mut()
                        .spawn((
                            Stockpile {
                                capacity: 2,
                                resource_type: None,
                            },
                            BelongsTo(equipment),
                            Sprite::default(),
                        ))
                        .id();
                    app.world_mut()
                        .resource_mut::<WorldMap>()
                        .register_stockpile_tile(grid, entity);
                    (grid, entity)
                })
                .collect();
            let mut window = Window {
                resolution: WindowResolution::new(100, 100),
                ..default()
            };
            window.set_cursor_position(Some(Vec2::new(25.0, 75.0)));
            let window = app.world_mut().spawn((PrimaryWindow, window)).id();
            let center = WorldMap::grid_to_world(10, 10);
            app.world_mut().spawn((
                MainCamera,
                Camera {
                    computed: ComputedCameraValues {
                        clip_from_view: Mat4::from_scale(Vec3::new(
                            TILE_SIZE.recip(),
                            TILE_SIZE.recip(),
                            1.0,
                        )),
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::splat(100),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
                GlobalTransform::from_translation(center.extend(0.0)),
            ));
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear_just_pressed(MouseButton::Left);
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(Vec2::new(75.0, 25.0)));
            app.update();
            let displayed = app
                .world()
                .resource::<ZoneRemovalPreviewState>()
                .displayed
                .as_ref()
                .unwrap();
            assert_eq!(
                displayed
                    .targets
                    .direct
                    .iter()
                    .map(|cell| cell.entity)
                    .collect::<Vec<_>>(),
                [targets[0].1]
            );
            assert!(displayed.targets.fragments.is_empty());

            if release == Release::Hud {
                app.world_mut()
                    .resource_mut::<UiInputState>()
                    .pointer_over_ui = true;
            }
            if release == Release::Paused {
                app.world_mut().resource_mut::<Time<Virtual>>().pause();
            }
            let expected = if release == Release::ReplacedCell {
                regular_cell(app.world_mut(), (10, 10), yard)
            } else {
                targets[0].1
            };
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            app.update();
            let cancel = release != Release::World;
            assert_eq!(app.world().get_entity(expected).is_ok(), cancel);
            assert_eq!(
                app.world().resource::<TaskContext>().0,
                TaskMode::ZoneRemoval(TaskModeZoneType::Stockpile, None),
            );
            assert_eq!(
                app.world()
                    .resource::<WorldMap>()
                    .stockpile_entity((10, 10)),
                cancel.then_some(expected)
            );
            for (grid, entity) in targets.into_iter().skip(1).chain(companions) {
                assert!(app.world().get_entity(entity).is_ok());
                assert_eq!(
                    app.world().resource::<WorldMap>().stockpile_entity(grid),
                    Some(entity)
                );
            }
            assert!(
                !app.world()
                    .resource::<ZoneRemovalPreviewState>()
                    .is_active()
            );
        }
    }
}
