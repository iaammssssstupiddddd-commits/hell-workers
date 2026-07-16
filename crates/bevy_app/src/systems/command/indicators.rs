use super::{
    AreaEditHandleKind, AreaEditHandleVisual, DesignationIndicator, TaskArea, TaskAreaIndicator,
    TaskMode,
};
use crate::app_contexts::TaskContext;
use crate::entities::familiar::Familiar;
use crate::interface::selection::SelectedEntity;
use crate::systems::jobs::Designation;
use crate::systems::visual::task_area_visual::{TaskAreaMaterial, TaskAreaVisual};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use std::collections::HashSet;

type TaskAreaIndicatorQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TaskAreaIndicator,
        &'static mut Transform,
        &'static mut Visibility,
        &'static MeshMaterial2d<TaskAreaMaterial>,
    ),
    (Without<Familiar>, With<TaskAreaVisual>),
>;

pub fn task_area_indicator_system(
    q_familiars: Query<(Entity, Ref<TaskArea>, &Familiar), With<Familiar>>,
    mut q_indicators: TaskAreaIndicatorQuery,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TaskAreaMaterial>>,
) {
    let mut indexed_owners = HashSet::new();
    for (indicator_entity, indicator, mut transform, mut visibility, material_handle) in
        q_indicators.iter_mut()
    {
        if let Ok((_, task_area, _)) = q_familiars.get(indicator.0) {
            indexed_owners.insert(indicator.0);
            if task_area.is_changed() {
                let area = task_area.as_ref();
                let translation = area.center().extend(0.2);
                let scale = area.size().extend(1.0);
                if transform.translation != translation {
                    transform.translation = translation;
                }
                if transform.scale != scale {
                    transform.scale = scale;
                }
                if *visibility != Visibility::Visible {
                    *visibility = Visibility::Visible;
                }
                if let Some(mut material) = materials.get_mut(&material_handle.0)
                    && material.size != area.size()
                {
                    material.size = area.size();
                }
            }
        } else {
            commands.entity(indicator_entity).try_despawn();
        }
    }

    // The first frame of a TaskArea is the only time a missing indicator must
    // be created. Existing indicators are owner-linked above, so this avoids
    // the former Familiar × indicator scan every visual frame.
    for (fam_entity, task_area, familiar_comp) in q_familiars.iter() {
        if task_area.is_added() && !indexed_owners.contains(&fam_entity) {
            // 使い魔のコンポーネントに保持されている色インデックスを使用
            let palette = [
                LinearRgba::from(Color::srgba(0.7, 0.3, 1.0, 1.0)), // Purple (鮮明化)
                LinearRgba::from(Color::srgba(1.0, 0.7, 0.0, 1.0)), // Yellow-Orange (赤と区別)
                LinearRgba::from(Color::srgba(0.1, 1.0, 0.4, 1.0)), // Toxic Green
                LinearRgba::from(Color::srgba(1.0, 0.0, 0.1, 1.0)), // Red
            ];
            let color = palette[familiar_comp.color_index as usize % palette.len()];

            commands.spawn((
                TaskAreaIndicator(fam_entity),
                TaskAreaVisual {
                    familiar: fam_entity,
                },
                Mesh2d(meshes.add(Rectangle::default().mesh())),
                MeshMaterial2d(materials.add(TaskAreaMaterial {
                    color,
                    size: task_area.as_ref().size(),
                    state: 0,
                })),
                Transform::from_translation(task_area.as_ref().center().extend(0.2))
                    .with_scale(task_area.as_ref().size().extend(1.0)),
                Visibility::Visible,
            ));
        }
    }
}

pub fn area_edit_handles_visual_system(
    task_context: Res<TaskContext>,
    selected: Res<SelectedEntity>,
    q_task_areas: Query<&TaskArea, With<Familiar>>,
    mut q_handles: Query<(Entity, &AreaEditHandleVisual, &mut Transform)>,
    mut commands: Commands,
) {
    if !matches!(task_context.0, TaskMode::AreaSelection(_)) {
        for (handle_entity, _, _) in q_handles.iter_mut() {
            commands.entity(handle_entity).try_despawn();
        }
        return;
    }

    let Some(fam_entity) = selected.0 else {
        for (handle_entity, _, _) in q_handles.iter_mut() {
            commands.entity(handle_entity).try_despawn();
        }
        return;
    };
    let Ok(area) = q_task_areas.get(fam_entity) else {
        for (handle_entity, _, _) in q_handles.iter_mut() {
            commands.entity(handle_entity).try_despawn();
        }
        return;
    };

    let min = area.min();
    let max = area.max();
    let mid_x = (min.x + max.x) * 0.5;
    let mid_y = (min.y + max.y) * 0.5;
    let handle_size = (TILE_SIZE * 0.22).max(5.0);

    let handles = [
        (AreaEditHandleKind::TopLeft, Vec2::new(min.x, max.y)),
        (AreaEditHandleKind::Top, Vec2::new(mid_x, max.y)),
        (AreaEditHandleKind::TopRight, Vec2::new(max.x, max.y)),
        (AreaEditHandleKind::Right, Vec2::new(max.x, mid_y)),
        (AreaEditHandleKind::BottomRight, Vec2::new(max.x, min.y)),
        (AreaEditHandleKind::Bottom, Vec2::new(mid_x, min.y)),
        (AreaEditHandleKind::BottomLeft, Vec2::new(min.x, min.y)),
        (AreaEditHandleKind::Left, Vec2::new(min.x, mid_y)),
        (AreaEditHandleKind::Center, Vec2::new(mid_x, mid_y)),
    ];

    let mut existing_kinds = Vec::with_capacity(handles.len());
    for (handle_entity, handle, mut transform) in q_handles.iter_mut() {
        let Some((_, position)) = handles.iter().find(|(kind, _)| *kind == handle.kind) else {
            commands.entity(handle_entity).try_despawn();
            continue;
        };
        if handle.owner != fam_entity || existing_kinds.contains(&handle.kind) {
            commands.entity(handle_entity).try_despawn();
            continue;
        }
        existing_kinds.push(handle.kind);
        let translation = position.extend(0.36);
        if transform.translation != translation {
            transform.translation = translation;
        }
    }

    for (kind, pos) in handles {
        if existing_kinds.contains(&kind) {
            continue;
        }
        commands.spawn((
            AreaEditHandleVisual {
                owner: fam_entity,
                kind,
            },
            Sprite {
                color: if kind == AreaEditHandleKind::Center {
                    Color::srgba(1.0, 0.95, 0.5, 0.95)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.9)
                },
                custom_size: Some(Vec2::splat(handle_size)),
                ..default()
            },
            Transform::from_translation(pos.extend(0.36)),
        ));
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
                commands.entity(indicator_entity).try_despawn();
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
            Option<&hw_core::relationships::StoredIn>,
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
