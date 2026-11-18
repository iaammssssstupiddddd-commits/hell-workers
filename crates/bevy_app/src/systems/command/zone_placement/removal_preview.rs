use bevy::prelude::*;
use hw_core::WorldEpoch;
use hw_core::game_state::TaskModeZoneType;
use hw_world::StockpileRemovalTargets;
use hw_world::zones::AreaBounds;

#[derive(PartialEq)]
pub(super) struct RemovalPreview {
    pub epoch: WorldEpoch,
    pub kind: TaskModeZoneType,
    pub start: Vec2,
    pub area: AreaBounds,
    pub targets: StockpileRemovalTargets,
}

#[derive(Default, Resource)]
pub struct ZoneRemovalPreviewState {
    pub(super) displayed: Option<RemovalPreview>,
    original_colors: Vec<(Entity, Color)>,
}

impl ZoneRemovalPreviewState {
    pub(crate) fn is_active(&self) -> bool {
        self.displayed.is_some()
    }
}

pub(super) fn update_removal_preview(
    preview: RemovalPreview,
    q_sprites: &mut Query<&mut Sprite>,
    state: &mut ZoneRemovalPreviewState,
) {
    if state.displayed.as_ref() == Some(&preview) {
        return;
    }
    clear_removal_preview(q_sprites, state);
    let direct_color = Color::srgba(1.0, 0.2, 0.2, 0.4);
    let fragment_color = Color::srgba(1.0, 0.6, 0.0, 0.4);
    for (cells, color) in [
        (&preview.targets.direct, direct_color),
        (&preview.targets.fragments, fragment_color),
    ] {
        for cell in cells {
            if let Ok(mut sprite) = q_sprites.get_mut(cell.entity) {
                state.original_colors.push((cell.entity, sprite.color));
                sprite.color = color;
            }
        }
    }
    state.displayed = Some(preview);
}

pub(crate) fn clear_removal_preview(
    q_sprites: &mut Query<&mut Sprite>,
    state: &mut ZoneRemovalPreviewState,
) {
    state.displayed = None;
    for (entity, color) in state.original_colors.drain(..) {
        if let Ok(mut sprite) = q_sprites.get_mut(entity) {
            sprite.color = color;
        }
    }
}

pub(crate) fn reset_removal_preview(world: &mut World) {
    if let Some(mut state) = world.get_resource_mut::<ZoneRemovalPreviewState>() {
        let old = std::mem::take(&mut *state);
        for (entity, color) in old.original_colors {
            if let Some(mut sprite) = world.get_mut::<Sprite>(entity) {
                sprite.color = color;
            }
        }
    }
}
