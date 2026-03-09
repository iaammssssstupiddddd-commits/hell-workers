use crate::camera::MainCamera;
use crate::components::{HoverActionOverlay, MenuAction, MenuButton};
use crate::selection::HoveredEntity;
use bevy::prelude::*;
use hw_jobs::{Building, BuildingCategory};

const HOVER_ACTION_Y_OFFSET: f32 = 38.0;

pub fn hover_action_button_system(
    hovered: Res<HoveredEntity>,
    q_buildings: Query<(Entity, &Building, &GlobalTransform)>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q_overlay: Query<(
        &mut HoverActionOverlay,
        &mut Node,
        &mut MenuButton,
        &Interaction,
    )>,
) {
    let Ok((mut overlay, mut node, mut menu_button, interaction)) = q_overlay.single_mut() else {
        return;
    };

    let hovered_target = hovered.0.and_then(|entity| {
        q_buildings
            .get(entity)
            .ok()
            .filter(|(_, building, _)| building.kind.category() == BuildingCategory::Plant)
            .map(|(entity, _, _)| entity)
    });

    let effective_target = hovered_target.or_else(|| {
        if matches!(*interaction, Interaction::Hovered | Interaction::Pressed) {
            overlay.target
        } else {
            None
        }
    });

    let Some(target_entity) = effective_target else {
        node.display = Display::None;
        if !matches!(*interaction, Interaction::Hovered | Interaction::Pressed) {
            overlay.target = None;
        }
        return;
    };

    let Ok((_, _, target_transform)) = q_buildings.get(target_entity) else {
        node.display = Display::None;
        if !matches!(*interaction, Interaction::Hovered | Interaction::Pressed) {
            overlay.target = None;
        }
        return;
    };

    let Ok((camera, camera_transform)) = q_camera.single() else {
        node.display = Display::None;
        return;
    };

    let Ok(overlay_pos) =
        camera.world_to_viewport(camera_transform, target_transform.translation())
    else {
        node.display = Display::None;
        return;
    };

    node.left = Val::Px(overlay_pos.x);
    node.top = Val::Px(overlay_pos.y - HOVER_ACTION_Y_OFFSET);
    node.display = Display::Flex;
    menu_button.0 = MenuAction::MovePlantBuilding(target_entity);
    overlay.target = Some(target_entity);
}
