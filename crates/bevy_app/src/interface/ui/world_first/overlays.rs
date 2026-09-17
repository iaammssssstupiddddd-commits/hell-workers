//! Presentation-only map layers. No allocation, pathfinding or room detection is changed.
use crate::entities::familiar::FamiliarRangeIndicator;
use bevy::ecs::system::SystemParam;
use bevy::gizmos::config::GizmoConfigGroup;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::game_state::{TaskMode, TaskModeZoneType};
use hw_core::visual_mirror::SoulTaskVisualState;
use hw_ui::world_view::{WorldView, WorldViewState};
use hw_visual::site_yard_visual::SiteYardBoundaryVisual;
use hw_visual::task_area_visual::TaskAreaVisual;
use hw_world::room_detection::RoomOverlayTile;

#[derive(Default, Reflect, GizmoConfigGroup)]
pub(crate) struct WorldViewGizmos;

pub(crate) fn sync_tool_view(
    task: Res<crate::app_contexts::TaskContext>,
    mut view: ResMut<WorldViewState>,
) {
    let temporary = match task.0 {
        TaskMode::AreaSelection(_) => Some(WorldView::Duties),
        TaskMode::ZonePlacement(TaskModeZoneType::Stockpile, _)
        | TaskMode::StockpilePolicyEdit(_) => Some(WorldView::Storage),
        TaskMode::ZonePlacement(TaskModeZoneType::Yard, _) => Some(WorldView::Duties),
        _ => None,
    };
    if view.temporary != temporary {
        view.temporary = temporary;
    }
}

type OverlayQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Has<TaskAreaVisual>,
        Has<FamiliarRangeIndicator>,
        Has<SiteYardBoundaryVisual>,
        Option<&'static ChildOf>,
        Has<RoomOverlayTile>,
    ),
    Or<(
        With<TaskAreaVisual>,
        With<FamiliarRangeIndicator>,
        With<SiteYardBoundaryVisual>,
        With<RoomOverlayTile>,
    )>,
>;

pub(crate) fn world_overlay_visibility(
    view: Res<WorldViewState>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut overlays: OverlayQuery,
) {
    for (mut visibility, area, aura, boundary, parent, room) in &mut overlays {
        // Established work areas, command auras and Site/Yard borders are
        // persistent world information, independent of the optional map layer.
        let visible = if area || aura || boundary {
            true
        } else {
            room && (view.effective() == WorldView::Rooms
                || parent.is_some_and(|parent| selected.0 == Some(parent.parent())))
        };
        let next = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::selection::SelectedEntity;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn established_areas_stay_visible_without_selection_and_across_layers() {
        let mut world = World::new();
        world.init_resource::<WorldViewState>();
        world.init_resource::<SelectedEntity>();
        let owner = world.spawn_empty().id();
        let area = world
            .spawn((TaskAreaVisual { familiar: owner }, Visibility::Hidden))
            .id();
        let aura = world
            .spawn((FamiliarRangeIndicator(owner), Visibility::Hidden))
            .id();
        let boundary = world
            .spawn((SiteYardBoundaryVisual, Visibility::Hidden))
            .id();
        for layer in WorldView::ALL {
            world.resource_mut::<WorldViewState>().manual = layer;
            world.run_system_once(world_overlay_visibility).unwrap();
            for entity in [area, aura, boundary] {
                assert_eq!(
                    *world.get::<Visibility>(entity).unwrap(),
                    Visibility::Visible
                );
            }
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct MapLayerInputs<'w, 's> {
    stockpiles: Query<'w, 's, &'static GlobalTransform, With<hw_logistics::Stockpile>>,
    consumers: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            Option<&'static hw_energy::PowerSupplyState>,
        ),
        With<hw_energy::PowerConsumer>,
    >,
    tasks: Query<'w, 's, (&'static GlobalTransform, &'static SoulTaskVisualState)>,
    targets: Query<'w, 's, &'static GlobalTransform>,
}

pub(crate) fn draw_world_view(
    view: Res<WorldViewState>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    inputs: MapLayerInputs,
    mut gizmos: Gizmos<WorldViewGizmos>,
    world_map: Res<hw_world::WorldMap>,
) {
    if let Some(selected) = selected.0
        && let Some((p, size)) =
            hw_visual::selection_indicator::target_geometry(selected, &world_map, &inputs.targets)
    {
        let h = size * 0.5;
        let points = [
            p + Vec2::new(-h.x, -h.y),
            p + Vec2::new(h.x, -h.y),
            p + h,
            p + Vec2::new(-h.x, h.y),
        ];
        for index in 0..4 {
            gizmos.line_2d(
                points[index],
                points[(index + 1) % 4],
                Color::srgb(1.0, 0.85, 0.45),
            );
        }
    }
    if view.effective() == WorldView::Storage {
        for transform in &inputs.stockpiles {
            let p = transform.translation().truncate();
            let h = TILE_SIZE * 0.46;
            let points = [
                p + Vec2::new(-h, -h),
                p + Vec2::new(h, -h),
                p + Vec2::new(h, h),
                p + Vec2::new(-h, h),
            ];
            for index in 0..4 {
                gizmos.line_2d(
                    points[index],
                    points[(index + 1) % 4],
                    Color::srgb(0.35, 0.85, 1.0),
                );
            }
        }
    }
    if view.effective() == WorldView::Power {
        for (transform, supply) in &inputs.consumers {
            let p = transform.translation().truncate();
            if supply == Some(&hw_energy::PowerSupplyState::Supplied) {
                gizmos.circle_2d(p, TILE_SIZE * 0.45, Color::WHITE);
            } else {
                let r = TILE_SIZE * 0.35;
                for y in [-r, r] {
                    gizmos.line_2d(
                        p + Vec2::new(-r, y),
                        p + Vec2::new(r, -y),
                        Color::srgb(1.0, 0.6, 0.25),
                    );
                }
            }
        }
    }
    // Only show the selected Soul's committed visual target. This is not a route prediction.
    if let Some(selected) = selected.0
        && let Ok((source, task)) = inputs.tasks.get(selected)
        && let Some(target) = task.bucket_link.or(task.link_target)
        && let Ok(target) = inputs.targets.get(target)
    {
        gizmos.line_2d(
            source.translation().truncate(),
            target.translation().truncate(),
            Color::WHITE,
        );
        gizmos.circle_2d(target.translation().truncate(), 4.0, Color::WHITE);
    }
}
