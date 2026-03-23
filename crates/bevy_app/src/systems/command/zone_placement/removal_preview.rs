use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_world::identify_removal_targets;
use hw_world::zones::AreaBounds;
use std::collections::HashSet;

#[derive(Default, Resource)]
pub struct ZoneRemovalPreviewState {
    direct: HashSet<(i32, i32)>,
    fragments: HashSet<(i32, i32)>,
}

impl ZoneRemovalPreviewState {
    pub(crate) fn clear(&mut self) {
        self.direct.clear();
        self.fragments.clear();
    }
}

pub(crate) fn update_removal_preview(
    world_map: &WorldMap,
    area: &AreaBounds,
    q_sprites: &mut Query<&mut Sprite>,
    state: &mut ZoneRemovalPreviewState,
) {
    let (direct, fragments) = identify_removal_targets(world_map, area);
    let next_direct: HashSet<(i32, i32)> = direct.into_iter().collect();
    let next_fragments: HashSet<(i32, i32)> = fragments.into_iter().collect();

    let prev_direct = state.direct.clone();
    let prev_fragments = state.fragments.clone();

    for grid in prev_direct.difference(&next_direct) {
        if !next_fragments.contains(grid) {
            set_stockpile_color(world_map, q_sprites, grid, stockpile_default_color());
        }
    }

    for grid in prev_fragments.difference(&next_fragments) {
        if !next_direct.contains(grid) {
            set_stockpile_color(world_map, q_sprites, grid, stockpile_default_color());
        }
    }

    for grid in next_direct.difference(&state.direct) {
        set_stockpile_color(world_map, q_sprites, grid, stockpile_removal_color());
    }

    for grid in next_fragments.difference(&state.fragments) {
        set_stockpile_color(world_map, q_sprites, grid, stockpile_fragment_color());
    }

    state.direct = next_direct;
    state.fragments = next_fragments;
}

pub(crate) fn clear_removal_preview(
    world_map: &WorldMap,
    q_sprites: &mut Query<&mut Sprite>,
    state: &mut ZoneRemovalPreviewState,
) {
    for grid in state.direct.iter().chain(state.fragments.iter()) {
        set_stockpile_color(world_map, q_sprites, grid, stockpile_default_color());
    }

    state.clear();
}

fn set_stockpile_color(
    world_map: &WorldMap,
    q_sprites: &mut Query<&mut Sprite>,
    grid: &(i32, i32),
    color: Color,
) {
    if let Some(entity) = world_map.stockpile_entity(*grid) {
        if let Ok(mut sprite) = q_sprites.get_mut(entity) {
            sprite.color = color;
        }
    }
}

fn stockpile_default_color() -> Color {
    Color::srgba(1.0, 1.0, 0.0, 0.2)
}

fn stockpile_removal_color() -> Color {
    Color::srgba(1.0, 0.2, 0.2, 0.4)
}

fn stockpile_fragment_color() -> Color {
    Color::srgba(1.0, 0.6, 0.0, 0.4)
}
