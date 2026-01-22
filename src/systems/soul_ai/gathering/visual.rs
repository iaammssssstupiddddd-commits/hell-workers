use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::DamnedSoul;
use crate::systems::soul_ai::gathering::*;

/// 集会オーラのサイズと位置の更新システム
pub fn gathering_visual_update_system(
    q_spots: Query<(Entity, &GatheringSpot, &GatheringVisuals), Changed<GatheringSpot>>,
    mut q_visuals: Query<
        (&mut Sprite, &mut Transform, &mut Visibility),
        (Without<DamnedSoul>, Without<ParticipatingIn>),
    >,
) {
    for (_spot_entity, spot, visuals) in q_spots.iter() {
        // ビジュアルの更新 (サイズのみ - 位置はスポーン時のcenterを維持)
        let target_size = calculate_aura_size(spot.participants);
        let target_pos = spot.center.extend(Z_AURA);
        let target_obj_pos = spot.center.extend(Z_ITEM);

        // オーラの更新 (常に表示)
        if let Ok((mut sprite, mut transform, mut visibility)) =
            q_visuals.get_mut(visuals.aura_entity)
        {
            let target_size_vec = Some(Vec2::splat(target_size));
            if sprite.custom_size != target_size_vec {
                sprite.custom_size = target_size_vec;
            }
            if transform.translation != target_pos {
                transform.translation = target_pos;
            }
            if *visibility != Visibility::Inherited {
                *visibility = Visibility::Inherited;
            }
        }

        // 中心オブジェクトの更新 (人数が2人以上の時のみ表示)
        if let Some(obj_entity) = visuals.object_entity {
            if let Ok((_, mut transform, mut visibility)) = q_visuals.get_mut(obj_entity) {
                if transform.translation != target_obj_pos {
                    transform.translation = target_obj_pos;
                }

                // 1人以下（0人の猶予期間を含む）なら非表示
                let target_visibility = if spot.participants < 2 {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };

                if *visibility != target_visibility {
                    *visibility = target_visibility;
                }
            }
        }
    }
}

/// 集会スポットホバー時に参加者との間に紫の線を引くデバッグシステム
pub fn gathering_debug_visualization_system(
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    hovered_entity: Res<crate::interface::selection::HoveredEntity>,
    q_spots: Query<(Entity, &GatheringSpot)>,
    q_participants: Query<(&GlobalTransform, &ParticipatingIn), With<DamnedSoul>>,
    mut gizmos: Gizmos,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };

    let cursor_world_pos = window.cursor_position().and_then(|cursor_pos| {
        camera
            .viewport_to_world_2d(camera_transform, cursor_pos)
            .ok()
    });

    // 表示対象のスポットIDを保持するセット
    let mut target_spots = std::collections::HashSet::new();

    // 1. マウス座標がスポットの中心に近いかチェック (1タイル以内)
    if let Some(world_pos) = cursor_world_pos {
        for (entity, spot) in q_spots.iter() {
            if spot.center.distance(world_pos) < TILE_SIZE {
                target_spots.insert(entity);
            }
        }
    }

    // 2. もしSoulをホバーしていたら、そのSoulが参加しているスポットを対象にする
    if let Some(hovered) = hovered_entity.0 {
        if let Ok((_, participating_in)) = q_participants.get(hovered) {
            target_spots.insert(participating_in.0);
        }
    }

    // 対象のスポットをすべて描画
    for spot_entity in target_spots {
        if let Ok((_, spot)) = q_spots.get(spot_entity) {
            let center = spot.center;

            for (soul_transform, participating_in) in q_participants.iter() {
                if participating_in.0 == spot_entity {
                    let soul_pos = soul_transform.translation().truncate();
                    // 紫の線とドット
                    gizmos.line_2d(center, soul_pos, Color::srgba(0.8, 0.4, 1.0, 0.8));
                    gizmos.circle_2d(soul_pos, 4.0, Color::srgba(0.8, 0.4, 1.0, 0.6));
                }
            }

            // 中心に目立つ円を描く
            gizmos.circle_2d(center, 16.0, Color::srgba(0.8, 0.4, 1.0, 1.0));
        }
    }
}
