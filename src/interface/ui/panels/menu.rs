//! メニュー表示制御

use crate::interface::ui::components::*;
use bevy::prelude::*;

pub fn menu_visibility_system(
    menu_state: Res<MenuState>,
    mut q_architect: Query<
        &mut Node,
        (
            With<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
            Without<OrdersSubMenu>,
        ),
    >,
    mut q_zones: Query<
        &mut Node,
        (
            With<ZonesSubMenu>,
            Without<ArchitectSubMenu>,
            Without<OrdersSubMenu>,
        ),
    >,
    mut q_orders: Query<
        &mut Node,
        (
            With<OrdersSubMenu>,
            Without<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
        ),
    >,
) {
    if let Ok(mut node) = q_architect.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Architect) {
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
}
