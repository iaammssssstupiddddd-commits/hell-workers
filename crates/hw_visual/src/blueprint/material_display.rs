//! 資材アイコン・カウンター表示システム

use bevy::prelude::ChildOf;
use bevy::prelude::*;

use super::components::{MaterialCounter, MaterialIcon};
use super::{COUNTER_TEXT_OFFSET, MATERIAL_ICON_X_OFFSET, MATERIAL_ICON_Y_OFFSET};
use crate::handles::MaterialIconHandles;
use hw_core::logistics::ResourceType;
use hw_core::visual_mirror::construction::BlueprintVisualState;

pub fn spawn_material_display_system(
    mut commands: Commands,
    handles: Res<MaterialIconHandles>,
    q_blueprints: Query<
        (Entity, &BlueprintVisualState),
        Added<super::components::BlueprintVisual>,
    >,
) {
    for (bp_entity, state) in q_blueprints.iter() {
        let mut i = 0;
        for (resource_type, _, _) in &state.material_counts {
            let icon_image = material_icon_for(&handles, *resource_type);

            let offset = Vec3::new(
                MATERIAL_ICON_X_OFFSET,
                MATERIAL_ICON_Y_OFFSET - (i as f32 * 14.0),
                0.1,
            );

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
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Left),
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
            let offset = Vec3::new(
                MATERIAL_ICON_X_OFFSET,
                MATERIAL_ICON_Y_OFFSET - (i as f32 * 14.0),
                0.1,
            );

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
                    Name::new(format!(
                        "MaterialIcon (Flexible {:?})",
                        accepted_types
                    )),
                ));

                parent.spawn((
                    MaterialCounter {
                        resource_type: proxy_resource_type,
                    },
                    Text2d::new("0/0"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Left),
                    Transform::from_translation(offset + COUNTER_TEXT_OFFSET),
                    Name::new(format!(
                        "MaterialCounter (Flexible {:?})",
                        accepted_types
                    )),
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

        if let Some((accepted_types, delivered, required)) = &state.flexible_material {
            if accepted_types.contains(&counter.resource_type) {
                let accepted = accepted_types
                    .iter()
                    .map(|resource_type| format!("{:?}", resource_type))
                    .collect::<Vec<_>>()
                    .join("/");
                text.0 = format!("{} {}/{}", accepted, delivered, required);
                continue;
            }
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
