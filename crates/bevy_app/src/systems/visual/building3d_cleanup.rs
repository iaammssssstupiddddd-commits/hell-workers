//! Building3dVisual クリーンアップ・マテリアル遷移システム
//!
//! - Building が削除された時、対応する Building3dVisual エンティティを despawn する。
//! - Building が仮設→本設に遷移した時、Building3dVisual のマテリアルを通常色に差し替える。

use crate::plugins::startup::Building3dHandles;
use bevy::prelude::*;
use hw_jobs::Building;
use hw_visual::visual3d::Building3dVisual;

/// Building エンティティ削除時に対応する Building3dVisual を despawn する。
pub fn cleanup_building_3d_visuals_system(
    mut commands: Commands,
    mut removed: RemovedComponents<Building>,
    q_visuals: Query<(Entity, &Building3dVisual)>,
) {
    for removed_entity in removed.read() {
        for (visual_entity, visual) in q_visuals.iter() {
            if visual.owner == removed_entity {
                commands.entity(visual_entity).despawn();
            }
        }
    }
}

/// 仮設壁が本設壁に遷移した時に Building3dVisual のマテリアルを通常色に差し替える。
pub fn sync_provisional_wall_material_system(
    handles_3d: Res<Building3dHandles>,
    q_buildings: Query<(Entity, &Building), Changed<Building>>,
    q_visuals: Query<(Entity, &Building3dVisual)>,
    mut q_materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    for (building_entity, building) in q_buildings.iter() {
        // 仮設から本設への遷移のみ対象
        if building.is_provisional {
            continue;
        }
        if !matches!(building.kind, hw_jobs::BuildingType::Wall) {
            continue;
        }

        for (visual_entity, visual) in q_visuals.iter() {
            if visual.owner != building_entity {
                continue;
            }
            if let Ok(mut mat) = q_materials.get_mut(visual_entity) {
                mat.0 = handles_3d.wall_material.clone();
            }
        }
    }
}
