use super::{DesignationIndicator, TaskArea, TaskAreaIndicator};
use crate::entities::familiar::Familiar;
use crate::systems::jobs::Designation;
use bevy::prelude::*;

pub fn task_area_indicator_system(
    q_familiars: Query<(Entity, &Transform, &TaskArea), With<Familiar>>,
    mut q_indicators: Query<
        (
            Entity,
            &TaskAreaIndicator,
            &mut Transform,
            &mut Visibility,
            &mut Sprite,
        ),
        Without<Familiar>,
    >,
    mut commands: Commands,
) {
    for (indicator_entity, indicator, mut transform, mut visibility, mut sprite) in
        q_indicators.iter_mut()
    {
        if let Ok((_, _, task_area)) = q_familiars.get(indicator.0) {
            transform.translation = task_area.center().extend(0.2);
            sprite.custom_size = Some(task_area.size());
            *visibility = Visibility::Visible;
        } else {
            commands.entity(indicator_entity).despawn();
        }
    }

    for (fam_entity, _, task_area) in q_familiars.iter() {
        let has_indicator = q_indicators
            .iter()
            .any(|(_, ind, _, _, _)| ind.0 == fam_entity);

        if !has_indicator {
            commands.spawn((
                TaskAreaIndicator(fam_entity),
                Sprite {
                    color: Color::srgba(0.0, 1.0, 0.0, 0.15),
                    custom_size: Some(task_area.size()),
                    ..default()
                },
                Transform::from_translation(task_area.center().extend(0.2)),
            ));
        }
    }
}

pub fn update_designation_indicator_system(
    mut commands: Commands,
    mut removed: RemovedComponents<Designation>,
    q_indicators: Query<(Entity, &DesignationIndicator)>,
) {
    for entity in removed.read() {
        for (indicator_entity, indicator) in q_indicators.iter() {
            if indicator.0 == entity {
                commands.entity(indicator_entity).despawn();
            }
        }
    }
}

/// DesignationIndicator をターゲットに同期させるシステム
pub fn sync_designation_indicator_system(
    mut q_indicators: Query<(&DesignationIndicator, &mut Transform, &mut Visibility)>,
    q_targets: Query<
        (
            &Transform,
            &Visibility,
            Option<&crate::relationships::StoredIn>,
        ),
        Without<DesignationIndicator>,
    >,
    q_parents: Query<&Transform, (Without<DesignationIndicator>, Without<Designation>)>,
) {
    for (indicator, mut transform, mut visibility) in q_indicators.iter_mut() {
        if let Ok((target_transform, target_visibility, stored_in_opt)) = q_targets.get(indicator.0)
        {
            if let Some(stored_in) = stored_in_opt {
                // アイテムが何かに格納されている（インベントリ内など）場合、その親の座標に同期
                if let Ok(parent_transform) = q_parents.get(stored_in.0) {
                    transform.translation = parent_transform.translation.truncate().extend(0.5);
                    *visibility = Visibility::Visible;
                } else {
                    *visibility = Visibility::Hidden;
                }
            } else if *target_visibility != Visibility::Hidden {
                // 地面に置かれていて表示されている場合、アイテム自体の座標に同期
                transform.translation = target_transform.translation.truncate().extend(0.5);
                *visibility = Visibility::Visible;
            } else {
                // それ以外（非表示など）はインジケーターも隠す
                *visibility = Visibility::Hidden;
            }
        } else {
            // ターゲット消失
            *visibility = Visibility::Hidden;
        }
    }
}
