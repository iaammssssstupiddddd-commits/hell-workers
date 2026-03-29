use crate::systems::visual::elevation_view::{ElevationDirection, ElevationViewState};
use bevy::prelude::*;
use hw_ui::camera::MainCamera;
use hw_visual::SectionCut;

pub fn sync_section_cut_normal_system(
    elevation: Res<ElevationViewState>,
    q_main_camera: Query<Ref<Transform>, With<MainCamera>>,
    mut cut: ResMut<SectionCut>,
) {
    let camera_changed = q_main_camera.single().is_ok_and(|cam| cam.is_changed());
    if !elevation.is_changed() && !camera_changed {
        return;
    }

    if let Ok(cam) = q_main_camera.single() {
        cut.position = Vec3::new(cam.translation.x, 0.0, -cam.translation.y);
    }

    match elevation.direction {
        ElevationDirection::TopDown => {
            cut.active = false;
        }
        ElevationDirection::North => {
            cut.normal = Vec3::NEG_Z;
            cut.active = true;
        }
        ElevationDirection::South => {
            cut.normal = Vec3::Z;
            cut.active = true;
        }
        ElevationDirection::East => {
            cut.normal = Vec3::NEG_X;
            cut.active = true;
        }
        ElevationDirection::West => {
            cut.normal = Vec3::X;
            cut.active = true;
        }
    }
}
