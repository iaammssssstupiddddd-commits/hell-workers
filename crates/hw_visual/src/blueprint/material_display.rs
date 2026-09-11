//! 資材アイコン・カウンター表示システム

use bevy::prelude::ChildOf;
use bevy::prelude::*;

use super::components::{MaterialCounter, MaterialIcon};
use super::{COUNTER_TEXT_OFFSET, MATERIAL_ICON_X_OFFSET, MATERIAL_ICON_Y_OFFSET};
use crate::handles::MaterialIconHandles;
use hw_core::constants::TILE_SIZE;
use hw_core::logistics::ResourceType;
use hw_core::visual_mirror::construction::BlueprintVisualState;

fn material_display_offset(state: &BlueprintVisualState, row: usize) -> Vec3 {
    // Door blueprints sit between structural supports. Keep their counters
    // below the footprint and its southern support instead of painting them
    // over either the east-west or north-south support pair.
    let (x, y) = if state.is_wall_or_door && !state.is_plain_wall {
        (-MATERIAL_ICON_X_OFFSET * 0.5, -TILE_SIZE * 2.0)
    } else {
        (MATERIAL_ICON_X_OFFSET, MATERIAL_ICON_Y_OFFSET)
    };
    Vec3::new(x, y - row as f32 * 14.0, 0.1)
}

pub fn spawn_material_display_system(
    mut commands: Commands,
    handles: Res<MaterialIconHandles>,
    q_blueprints: Query<(Entity, &BlueprintVisualState), Added<super::components::BlueprintVisual>>,
) {
    for (bp_entity, state) in q_blueprints.iter() {
        let mut i = 0;
        for (resource_type, _, _) in &state.material_counts {
            let icon_image = material_icon_for(&handles, *resource_type);

            let offset = material_display_offset(state, i);

            commands.entity(bp_entity).with_children(|parent| {
                parent.spawn((
                    MaterialIcon {
                        _resource_type: *resource_type,
                    },
                    Sprite {
                        image: icon_image,
                        custom_size: Some(Vec2::splat(12.0)),
                        ..default()
                    },
                    Transform::from_translation(offset),
                    Name::new(format!("MaterialIcon ({:?})", resource_type)),
                ));

                parent.spawn((
                    MaterialCounter {
                        resource_type: *resource_type,
                    },
                    Text2d::new("0/0"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Left),
                    Transform::from_translation(offset + COUNTER_TEXT_OFFSET),
                    Name::new(format!("MaterialCounter ({:?})", resource_type)),
                ));
            });

            i += 1;
        }

        if let Some((accepted_types, _, _)) = &state.flexible_material
            && let Some(&proxy_resource_type) = accepted_types.first()
        {
            let icon_image = material_icon_for(&handles, proxy_resource_type);
            let offset = material_display_offset(state, i);

            commands.entity(bp_entity).with_children(|parent| {
                parent.spawn((
                    MaterialIcon {
                        _resource_type: proxy_resource_type,
                    },
                    Sprite {
                        image: icon_image,
                        custom_size: Some(Vec2::splat(12.0)),
                        ..default()
                    },
                    Transform::from_translation(offset),
                    Name::new(format!("MaterialIcon (Flexible {:?})", accepted_types)),
                ));

                parent.spawn((
                    MaterialCounter {
                        resource_type: proxy_resource_type,
                    },
                    Text2d::new("0/0"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Left),
                    Transform::from_translation(offset + COUNTER_TEXT_OFFSET),
                    Name::new(format!("MaterialCounter (Flexible {:?})", accepted_types)),
                ));
            });
        }
    }
}

pub fn update_material_counter_system(
    q_blueprints: Query<&BlueprintVisualState>,
    mut q_counters: Query<(&MaterialCounter, &ChildOf, &mut Text2d)>,
) {
    for (counter, child_of, mut text) in q_counters.iter_mut() {
        let Ok(state) = q_blueprints.get(child_of.parent()) else {
            continue;
        };

        if let Some((accepted_types, delivered, required)) = &state.flexible_material
            && accepted_types.contains(&counter.resource_type)
        {
            let accepted = accepted_types
                .iter()
                .map(|resource_type| format!("{:?}", resource_type))
                .collect::<Vec<_>>()
                .join("/");
            text.0 = format!("{} {}/{}", accepted, delivered, required);
            continue;
        }

        if let Some((_, delivered, required)) = state
            .material_counts
            .iter()
            .find(|(rt, _, _)| *rt == counter.resource_type)
        {
            text.0 = format!("{}/{}", delivered, required);
        }
    }
}

fn material_icon_for(handles: &MaterialIconHandles, resource_type: ResourceType) -> Handle<Image> {
    match resource_type {
        ResourceType::Wood => handles.wood_small.clone(),
        ResourceType::Rock => handles.rock_small.clone(),
        ResourceType::Water => handles.water_small.clone(),
        ResourceType::Sand => handles.sand_small.clone(),
        ResourceType::Bone => handles.bone_small.clone(),
        ResourceType::StasisMud => handles.stasis_mud_small.clone(),
        _ => handles.rock_small.clone(),
    }
}

pub fn cleanup_material_display_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, With<BlueprintVisualState>>,
    q_icons: Query<(Entity, &ChildOf, &MaterialIcon)>,
    q_counters: Query<(Entity, &ChildOf, &MaterialCounter)>,
) {
    let bp_entities: std::collections::HashSet<Entity> = q_blueprints.iter().collect();

    for (entity, child_of, _) in q_icons.iter() {
        if !bp_entities.contains(&child_of.parent()) {
            commands.entity(entity).try_despawn();
        }
    }

    for (entity, child_of, _) in q_counters.iter() {
        if !bp_entities.contains(&child_of.parent()) {
            commands.entity(entity).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn door_material_rows_stay_below_the_footprint_and_clear_of_supports() {
        let door = BlueprintVisualState {
            is_wall_or_door: true,
            ..default()
        };
        for row in 0..2 {
            let icon = material_display_offset(&door, row);
            let counter = icon + COUNTER_TEXT_OFFSET;
            assert!(icon.y <= -TILE_SIZE * 2.0);
            assert!(counter.y <= -TILE_SIZE * 2.0);
            assert!(icon.x.abs() < TILE_SIZE * 0.5);
            assert!(counter.x.abs() < TILE_SIZE * 0.5);
        }
        assert_eq!(
            material_display_offset(&door, 0).y - material_display_offset(&door, 1).y,
            14.0
        );
    }

    #[test]
    fn non_door_material_rows_keep_the_existing_layout() {
        for state in [
            BlueprintVisualState::default(),
            BlueprintVisualState {
                is_wall_or_door: true,
                is_plain_wall: true,
                ..default()
            },
        ] {
            assert_eq!(
                material_display_offset(&state, 0),
                Vec3::new(MATERIAL_ICON_X_OFFSET, MATERIAL_ICON_Y_OFFSET, 0.1)
            );
        }
    }
}
