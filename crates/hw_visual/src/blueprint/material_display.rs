//! 資材アイコン・カウンター表示システム

use bevy::prelude::ChildOf;
use bevy::prelude::*;

use super::components::{MaterialCounter, MaterialIcon};
use super::{COUNTER_TEXT_OFFSET, MATERIAL_ICON_X_OFFSET, MATERIAL_ICON_Y_OFFSET};
use crate::handles::MaterialIconHandles;
use hw_core::logistics::ResourceType;
use hw_jobs::Blueprint;

pub fn spawn_material_display_system(
    mut commands: Commands,
    handles: Res<MaterialIconHandles>,
    q_blueprints: Query<
        (Entity, &Blueprint),
        (With<Blueprint>, Added<super::components::BlueprintVisual>),
    >,
) {
    for (bp_entity, bp) in q_blueprints.iter() {
        let mut i = 0;
        for (resource_type, _) in &bp.required_materials {
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

        if let Some(flexible) = &bp.flexible_material_requirement
            && let Some(&proxy_resource_type) = flexible.accepted_types.first()
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
                        flexible.accepted_types
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
                        flexible.accepted_types
                    )),
                ));
            });
        }
    }
}

pub fn update_material_counter_system(
    q_blueprints: Query<&Blueprint>,
    mut q_counters: Query<(&MaterialCounter, &ChildOf, &mut Text2d)>,
) {
    for (counter, child_of, mut text) in q_counters.iter_mut() {
        if let Ok(bp) = q_blueprints.get(child_of.parent()) {
            if let Some(flexible) = &bp.flexible_material_requirement
                && flexible.accepts(counter.resource_type)
            {
                let accepted = flexible
                    .accepted_types
                    .iter()
                    .map(|resource_type| format!("{:?}", resource_type))
                    .collect::<Vec<_>>()
                    .join("/");
                text.0 = format!(
                    "{} {}/{}",
                    accepted, flexible.delivered_total, flexible.required_total
                );
                continue;
            }

            let delivered = bp
                .delivered_materials
                .get(&counter.resource_type)
                .unwrap_or(&0);
            let required = bp
                .required_materials
                .get(&counter.resource_type)
                .unwrap_or(&0);
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
    q_blueprints: Query<Entity, With<Blueprint>>,
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
