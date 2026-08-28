use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::selection::{
    FamiliarMoveFeedback, SelectedEntity, SelectionHitKind, SelectionIndicator, WorldPointerTarget,
};
use hw_world::WorldMap;

#[derive(Component, Clone, Copy)]
pub struct HoverSelectionIndicator;

#[derive(Component)]
pub struct FamiliarDestinationMarker(Timer);

type SelectedIndicatorQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Transform, &'static mut Sprite), With<SelectionIndicator>>;
type HoverIndicatorQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Transform, &'static mut Sprite),
    With<HoverSelectionIndicator>,
>;

pub fn update_selection_indicator(
    selected: Res<SelectedEntity>,
    pointer: Res<WorldPointerTarget>,
    world_map: Res<WorldMap>,
    q_transforms: Query<&GlobalTransform>,
    mut indicators: ParamSet<(SelectedIndicatorQuery, HoverIndicatorQuery)>,
    mut commands: Commands,
) {
    sync_indicator(
        selected.0,
        Color::srgba(1.0, 0.92, 0.2, 0.45),
        &world_map,
        &q_transforms,
        &mut indicators.p0(),
        &mut commands,
        SelectionIndicator,
    );

    let hover_target = pointer
        .primary
        .filter(|candidate| Some(candidate.entity) != selected.0);
    let hover_color = hover_target.map_or(Color::NONE, |candidate| match candidate.hit {
        SelectionHitKind::Direct => Color::srgba(0.85, 0.95, 1.0, 0.28),
        SelectionHitKind::Snapped => Color::srgba(0.2, 0.9, 1.0, 0.5),
    });
    sync_indicator(
        hover_target.map(|candidate| candidate.entity),
        hover_color,
        &world_map,
        &q_transforms,
        &mut indicators.p1(),
        &mut commands,
        HoverSelectionIndicator,
    );
}

pub fn update_familiar_destination_marker(
    time: Res<Time>,
    feedback: Res<FamiliarMoveFeedback>,
    mut last_revision: Local<u64>,
    mut markers: Query<(Entity, &mut FamiliarDestinationMarker, &mut Sprite)>,
    mut commands: Commands,
) {
    if feedback.revision != *last_revision {
        *last_revision = feedback.revision;
        for (entity, _, _) in &mut markers {
            commands.entity(entity).despawn();
        }
        if feedback.revision != 0 {
            commands.spawn((
                FamiliarDestinationMarker(Timer::from_seconds(0.7, TimerMode::Once)),
                Sprite {
                    color: Color::srgba(0.25, 1.0, 0.65, 0.65),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.45)),
                    ..default()
                },
                Transform::from_translation(feedback.destination.extend(Z_SELECTION)),
            ));
        }
    }

    for (entity, mut marker, mut sprite) in &mut markers {
        marker.0.tick(time.delta());
        let remaining = marker.0.fraction_remaining();
        sprite.color = Color::srgba(0.25, 1.0, 0.65, 0.65 * remaining);
        if marker.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn sync_indicator<Marker: Component + Copy>(
    target: Option<Entity>,
    color: Color,
    world_map: &WorldMap,
    q_transforms: &Query<&GlobalTransform>,
    q_indicator: &mut Query<(Entity, &mut Transform, &mut Sprite), With<Marker>>,
    commands: &mut Commands,
    marker: Marker,
) {
    let Some(target) = target else {
        for (entity, _, _) in q_indicator.iter_mut() {
            commands.entity(entity).despawn();
        }
        return;
    };
    let Some((position, size)) = target_geometry(target, world_map, q_transforms) else {
        return;
    };
    if let Ok((_, mut transform, mut sprite)) = q_indicator.single_mut() {
        transform.translation = position.extend(Z_SELECTION);
        sprite.custom_size = Some(size);
        sprite.color = color;
    } else {
        commands.spawn((
            marker,
            Sprite {
                color,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_translation(position.extend(Z_SELECTION)),
        ));
    }
}

fn target_geometry(
    target: Entity,
    world_map: &WorldMap,
    q_transforms: &Query<&GlobalTransform>,
) -> Option<(Vec2, Vec2)> {
    let snapshot = world_map.snapshot_owner(target);
    let grids = if !snapshot.building_grids.is_empty() {
        snapshot.building_grids
    } else if !snapshot.floor_grids.is_empty() {
        snapshot.floor_grids
    } else {
        snapshot.stockpile_grids
    };
    if let Some((&first, rest)) = grids.split_first() {
        let (mut min, mut max) = (first, first);
        for &(x, y) in rest {
            min.0 = min.0.min(x);
            min.1 = min.1.min(y);
            max.0 = max.0.max(x);
            max.1 = max.1.max(y);
        }
        let min_world = WorldMap::grid_to_world(min.0, min.1);
        let max_world = WorldMap::grid_to_world(max.0, max.1);
        let center = (min_world + max_world) / 2.0;
        let size = Vec2::new(
            (max.0 - min.0 + 1) as f32 * TILE_SIZE,
            (max.1 - min.1 + 1) as f32 * TILE_SIZE,
        ) * 1.04;
        Some((center, size))
    } else {
        q_transforms.get(target).ok().map(|transform| {
            (
                transform.translation().truncate(),
                Vec2::splat(TILE_SIZE * 1.1),
            )
        })
    }
}
