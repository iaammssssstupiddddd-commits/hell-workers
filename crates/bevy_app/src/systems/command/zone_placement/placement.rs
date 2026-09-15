use super::plan::{ZonePlacementPreview, ZonePlan, ZonePreview, ZoneReject, build_zone_plan};
use crate::app_contexts::TaskContext;
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskMode;
use crate::systems::logistics::{BelongsTo, Stockpile};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::WorldEpoch;
use hw_core::constants::*;
use hw_core::game_state::PlayMode;
use hw_logistics::StockpilePolicy;
use hw_ui::camera::MainCamera;
use hw_ui::notifications::{NotificationRetention, NotificationSeverity, UserFacingNotification};
use hw_world::zones::Site;
use hw_world::zones::{AreaBounds, Yard};

const STOCKPILE_CELL_CAPACITY: usize = 10;

#[derive(SystemParam)]
pub struct ZonePlacementInput<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<'w, UiInputState>,
    preview: ResMut<'w, ZonePlacementPreview>,
    epoch: Res<'w, WorldEpoch>,
    notifications: MessageWriter<'w, UserFacingNotification>,
}

pub fn zone_placement_system(
    mut input: ZonePlacementInput,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
    q_yards: Query<(Entity, &Yard)>,
    q_sites: Query<&Site>,
) {
    let TaskMode::ZonePlacement(kind, start) = task_context.0 else {
        input.preview.0 = None;
        return;
    };
    let released = input.buttons.just_released(MouseButton::Left);
    // Release consumes the gesture even if capture or a missing cursor prevents commit.
    if released {
        task_context.0 = TaskMode::ZonePlacement(kind, None);
    }
    if input.ui_input_state.world_input_blocked() {
        input.preview.0 = None;
        return;
    }
    if input.buttons.just_pressed(MouseButton::Right) {
        task_context.0 = TaskMode::None;
        next_play_mode.set(PlayMode::Normal);
        input.preview.0 = None;
        return;
    }
    let Some(world_pos) = super::world_cursor_pos(&input.q_window, &input.q_camera) else {
        input.preview.0 = None;
        return;
    };
    let end = WorldMap::snap_to_grid_edge(world_pos);
    if input.buttons.just_pressed(MouseButton::Left) {
        task_context.0 = TaskMode::ZonePlacement(kind, Some(end));
        input.preview.0 = None;
        return;
    }
    let Some(start) = start else {
        input.preview.0 = None;
        return;
    };
    let area = AreaBounds::from_points(start, end);
    let yards: Vec<_> = q_yards
        .iter()
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect();
    let sites: Vec<_> = q_sites.iter().cloned().collect();
    let result = build_zone_plan(kind, start, &area, &world_map, &yards, &sites);
    if !released {
        input.preview.0 = Some(ZonePreview {
            epoch: *input.epoch,
            kind,
            start,
            area,
            result,
        });
        return;
    }
    let previous = input.preview.0.take();
    let result = result.and_then(|plan| {
        let matches_preview = previous.as_ref().is_some_and(|preview| {
            preview.epoch == *input.epoch
                && preview.kind == kind
                && preview.start == start
                && preview.area == area
                && preview.result.as_ref() == Ok(&plan)
        });
        if matches_preview {
            Ok(plan)
        } else {
            Err(ZoneReject::PreviewChanged)
        }
    });
    let (severity, message) = match result {
        Ok(plan) => {
            let message = plan.summary();
            apply_plan(&mut commands, &mut world_map, plan);
            (NotificationSeverity::Success, message)
        }
        Err(reason) => (NotificationSeverity::Warning, reason.label().to_owned()),
    };
    input.notifications.write(UserFacingNotification::new(
        "zone-placement",
        severity,
        "Zone配置",
        message,
        NotificationRetention::ToastOnly,
    ));
}

fn apply_plan(commands: &mut Commands, world_map: &mut WorldMap, plan: ZonePlan) {
    match plan {
        ZonePlan::Yard { owner, bounds, .. } => {
            commands.entity(owner).insert(Yard {
                min: bounds.min,
                max: bounds.max,
            });
        }
        ZonePlan::Stockpile { cells, .. } => {
            for (grid, owner) in cells {
                let pos = WorldMap::grid_to_world(grid.0, grid.1);
                let entity = commands
                    .spawn((
                        Stockpile {
                            capacity: STOCKPILE_CELL_CAPACITY,
                            resource_type: None,
                        },
                        StockpilePolicy::for_capacity(STOCKPILE_CELL_CAPACITY),
                        BelongsTo(owner),
                        Sprite {
                            color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                            custom_size: Some(Vec2::splat(TILE_SIZE)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                        Name::new("Stockpile"),
                    ))
                    .id();
                world_map.register_stockpile_tile(grid, entity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::game_state::TaskModeZoneType;

    #[test]
    fn release_consumes_drag_when_cursor_is_missing_or_ui_captures_input() {
        for captured in [false, true] {
            let mut app = App::new();
            let kind = TaskModeZoneType::Stockpile;
            let start = Vec2::ZERO;
            app.init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<WorldMap>()
                .init_resource::<WorldEpoch>()
                .init_resource::<NextState<PlayMode>>()
                .insert_resource(TaskContext(TaskMode::ZonePlacement(kind, Some(start))))
                .insert_resource(UiInputState {
                    world_input_captured: captured,
                    ..default()
                })
                .insert_resource(ZonePlacementPreview(Some(ZonePreview {
                    epoch: WorldEpoch::default(),
                    kind,
                    start,
                    area: AreaBounds::from_points(start, Vec2::splat(TILE_SIZE)),
                    result: Err(ZoneReject::OutsideYard),
                })))
                .add_message::<UserFacingNotification>()
                .add_systems(Update, zone_placement_system);
            let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            buttons.press(MouseButton::Left);
            buttons.clear();
            buttons.release(MouseButton::Left);
            app.update();
            assert_eq!(
                app.world().resource::<TaskContext>().0,
                TaskMode::ZonePlacement(kind, None)
            );
            assert!(app.world().resource::<ZonePlacementPreview>().0.is_none());
            assert!(
                app.world()
                    .resource::<Messages<UserFacingNotification>>()
                    .is_empty()
            );
            assert_eq!(
                app.world_mut()
                    .query::<&Stockpile>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
    }
}
