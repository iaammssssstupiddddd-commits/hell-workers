use crate::camera::MainCamera;
use crate::components::{HoverActionOverlay, MenuAction, MenuButton};
use bevy::prelude::*;

const HOVER_ACTION_Y_OFFSET: f32 = 38.0;

/// Root-produced, domain-validated target for the hover action widget.
///
/// `hw_ui` deliberately stores only an entity id; the root adapter decides
/// whether the currently hovered simulation entity is a movable Plant.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoverActionTarget(pub Option<Entity>);

fn effective_target(
    candidate: Option<Entity>,
    latched: Option<Entity>,
    interaction: Interaction,
) -> Option<Entity> {
    candidate.or_else(|| {
        matches!(interaction, Interaction::Hovered | Interaction::Pressed).then_some(latched)?
    })
}

pub fn hover_action_button_system(
    target: Res<HoverActionTarget>,
    q_transforms: Query<&GlobalTransform>,
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

    let effective_target = effective_target(target.0, overlay.target, *interaction);

    let Some(target_entity) = effective_target else {
        node.display = Display::None;
        if !matches!(*interaction, Interaction::Hovered | Interaction::Pressed) {
            overlay.target = None;
        }
        return;
    };

    let Ok(target_transform) = q_transforms.get(target_entity) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_or_press_keeps_the_latched_target() {
        let target = Entity::from_bits(42);

        assert_eq!(
            effective_target(None, Some(target), Interaction::Hovered),
            Some(target)
        );
        assert_eq!(
            effective_target(None, Some(target), Interaction::Pressed),
            Some(target)
        );
        assert_eq!(
            effective_target(None, Some(target), Interaction::None),
            None
        );
    }

    #[test]
    fn fresh_candidate_replaces_the_latched_target() {
        let old = Entity::from_bits(41);
        let new = Entity::from_bits(42);

        assert_eq!(
            effective_target(Some(new), Some(old), Interaction::Hovered),
            Some(new)
        );
    }
}
