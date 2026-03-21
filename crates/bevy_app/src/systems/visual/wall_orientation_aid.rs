//! 壁の上面を見分けるための検証用補助ビジュアル

use crate::plugins::startup::Building3dHandles;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;

/// 壁 3D エンティティの子として、上面を示す薄い帯を追加する。
///
/// Camera3d の角度確認用の補助であり、壁の上面と側面を区別しやすくする。
pub fn attach_wall_orientation_aid(
    commands: &mut Commands,
    visual_entity: Entity,
    handles_3d: &Building3dHandles,
) {
    commands.entity(visual_entity).with_children(|parent| {
        parent.spawn((
            Mesh3d(handles_3d.wall_orientation_aid_mesh.clone()),
            MeshMaterial3d(handles_3d.wall_orientation_aid_material.clone()),
            Transform::from_xyz(0.0, TILE_SIZE * 0.56, 0.0),
            handles_3d.render_layers.clone(),
            Name::new("WallOrientationAid"),
        ));
    });
}
