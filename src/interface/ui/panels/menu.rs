//! メニュー表示制御

use crate::interface::ui::components::*;
use bevy::prelude::*;

pub fn menu_visibility_system(
    menu_state: Res<MenuState>,
    mut arch_category_state: ResMut<ArchitectCategoryState>,
    mut q_architect: Query<
        &mut Node,
        (
            With<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
            Without<OrdersSubMenu>,
            Without<DreamSubMenu>,
            Without<ArchitectCategoryListPanel>,
            Without<ArchitectBuildingPanel>,
        ),
    >,
    mut q_zones: Query<
        &mut Node,
        (
            With<ZonesSubMenu>,
            Without<ArchitectSubMenu>,
            Without<OrdersSubMenu>,
            Without<DreamSubMenu>,
            Without<ArchitectCategoryListPanel>,
            Without<ArchitectBuildingPanel>,
        ),
    >,
    mut q_orders: Query<
        &mut Node,
        (
            With<OrdersSubMenu>,
            Without<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
            Without<DreamSubMenu>,
            Without<ArchitectCategoryListPanel>,
            Without<ArchitectBuildingPanel>,
        ),
    >,
    mut q_dream: Query<
        &mut Node,
        (
            With<DreamSubMenu>,
            Without<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
            Without<OrdersSubMenu>,
            Without<ArchitectCategoryListPanel>,
            Without<ArchitectBuildingPanel>,
        ),
    >,
    mut q_category_panel: Query<
        &mut Node,
        (
            With<ArchitectCategoryListPanel>,
            Without<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
            Without<OrdersSubMenu>,
            Without<DreamSubMenu>,
            Without<ArchitectBuildingPanel>,
        ),
    >,
    mut q_building_panels: Query<
        (&mut Node, &ArchitectBuildingPanel),
        (
            Without<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
            Without<OrdersSubMenu>,
            Without<DreamSubMenu>,
            Without<ArchitectCategoryListPanel>,
        ),
    >,
) {
    let is_architect = matches!(*menu_state, MenuState::Architect);

    if let Ok(mut node) = q_architect.single_mut() {
        node.display = if is_architect {
            Display::Flex
        } else {
            Display::None
        };
    }

    // Architect メニューが閉じられたらカテゴリ状態をリセット
    if !is_architect {
        arch_category_state.0 = None;
    }

    // カテゴリ選択パネルは Architect が開いている間は常時表示
    if let Ok(mut node) = q_category_panel.single_mut() {
        node.display = if is_architect {
            Display::Flex
        } else {
            Display::None
        };
    }

    // 建物パネルの表示制御
    for (mut node, panel) in q_building_panels.iter_mut() {
        node.display = if is_architect && arch_category_state.0 == Some(panel.0) {
            Display::Flex
        } else {
            Display::None
        };
    }

    if let Ok(mut node) = q_zones.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Zones) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q_orders.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Orders) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q_dream.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Dream) {
            Display::Flex
        } else {
            Display::None
        };
    }
}
