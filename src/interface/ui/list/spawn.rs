//! エンティティリストUIノードのスポーン

mod familiar_section;
mod soul_row;

use super::{FamiliarRowViewModel, FamiliarSectionNodes, SoulRowViewModel};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

pub(super) fn spawn_soul_list_item_entity(
    commands: &mut Commands,
    parent_entity: Entity,
    soul_vm: &SoulRowViewModel,
    game_assets: &crate::assets::GameAssets,
    left_margin: f32,
    theme: &UiTheme,
) -> Entity {
    soul_row::spawn_soul_list_item_entity(
        commands,
        parent_entity,
        soul_vm,
        game_assets,
        left_margin,
        theme,
    )
}

pub(super) fn spawn_familiar_section(
    commands: &mut Commands,
    parent_container: Entity,
    familiar: &FamiliarRowViewModel,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> FamiliarSectionNodes {
    familiar_section::spawn_familiar_section(
        commands,
        parent_container,
        familiar,
        game_assets,
        theme,
    )
}

pub(super) fn spawn_empty_squad_hint_entity(
    commands: &mut Commands,
    parent_entity: Entity,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> Entity {
    familiar_section::spawn_empty_squad_hint_entity(commands, parent_entity, game_assets, theme)
}
