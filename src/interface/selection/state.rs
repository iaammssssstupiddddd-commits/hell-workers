use crate::constants::*;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct SelectedEntity(pub Option<Entity>);

#[derive(Resource, Default)]
pub struct HoveredEntity(pub Option<Entity>);

#[derive(Component)]
pub struct SelectionIndicator;

pub fn update_selection_indicator(
    selected: Res<SelectedEntity>,
    mut q_indicator: Query<(Entity, &mut Transform), With<SelectionIndicator>>,
    q_transforms: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    if let Some(entity) = selected.0 {
        if let Ok(target_transform) = q_transforms.get(entity) {
            if let Ok((_, mut indicator_transform)) = q_indicator.single_mut() {
                indicator_transform.translation = target_transform
                    .translation()
                    .truncate()
                    .extend(Z_SELECTION);
            } else {
                commands.spawn((
                    SelectionIndicator,
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 0.0, 0.4),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.1)),
                        ..default()
                    },
                    Transform::from_translation(
                        target_transform
                            .translation()
                            .truncate()
                            .extend(Z_SELECTION),
                    ),
                ));
            }
        }
    } else {
        for (indicator_entity, _) in q_indicator.iter() {
            commands.entity(indicator_entity).despawn();
        }
    }
}

pub fn cleanup_selection_references_system(
    mut selected_entity: ResMut<SelectedEntity>,
    mut hovered_entity: ResMut<HoveredEntity>,
    q_exists: Query<(), ()>,
) {
    if let Some(entity) = selected_entity.0
        && q_exists.get(entity).is_err()
    {
        selected_entity.0 = None;
    }

    if let Some(entity) = hovered_entity.0
        && q_exists.get(entity).is_err()
    {
        hovered_entity.0 = None;
    }
}
