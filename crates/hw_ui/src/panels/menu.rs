//! メニュー表示制御

use crate::components::*;
use bevy::prelude::*;

type ArchitectSubMenuQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<ArchitectSubMenu>,
        Without<ZonesSubMenu>,
        Without<OrdersSubMenu>,
        Without<DreamSubMenu>,
        Without<ArchitectCategoryListPanel>,
        Without<ArchitectBuildingPanel>,
    ),
>;

type ZonesSubMenuQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<ZonesSubMenu>,
        Without<ArchitectSubMenu>,
        Without<OrdersSubMenu>,
        Without<DreamSubMenu>,
        Without<ArchitectCategoryListPanel>,
        Without<ArchitectBuildingPanel>,
    ),
>;

type OrdersSubMenuQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<OrdersSubMenu>,
        Without<ArchitectSubMenu>,
        Without<ZonesSubMenu>,
        Without<DreamSubMenu>,
        Without<ArchitectCategoryListPanel>,
        Without<ArchitectBuildingPanel>,
    ),
>;

type DreamSubMenuQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<DreamSubMenu>,
        Without<ArchitectSubMenu>,
        Without<ZonesSubMenu>,
        Without<OrdersSubMenu>,
        Without<ArchitectCategoryListPanel>,
        Without<ArchitectBuildingPanel>,
    ),
>;

type CategoryPanelQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Node,
    (
        With<ArchitectCategoryListPanel>,
        Without<ArchitectSubMenu>,
        Without<ZonesSubMenu>,
        Without<OrdersSubMenu>,
        Without<DreamSubMenu>,
        Without<ArchitectBuildingPanel>,
    ),
>;

type BuildingPanelsQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Node, &'static ArchitectBuildingPanel),
    (
        Without<ArchitectSubMenu>,
        Without<ZonesSubMenu>,
        Without<OrdersSubMenu>,
        Without<DreamSubMenu>,
        Without<ArchitectCategoryListPanel>,
    ),
>;

use bevy::ecs::system::SystemParam;

#[derive(SystemParam)]
pub struct MenuNodeQueries<'w, 's> {
    pub q_architect: ArchitectSubMenuQuery<'w, 's>,
    pub q_zones: ZonesSubMenuQuery<'w, 's>,
    pub q_orders: OrdersSubMenuQuery<'w, 's>,
    pub q_dream: DreamSubMenuQuery<'w, 's>,
    pub q_category_panel: CategoryPanelQuery<'w, 's>,
    pub q_building_panels: BuildingPanelsQuery<'w, 's>,
}

pub fn menu_visibility_system(
    menu_state: Res<MenuState>,
    mut arch_category_state: ResMut<ArchitectCategoryState>,
    mut q: MenuNodeQueries,
) {
    let is_architect = matches!(*menu_state, MenuState::Architect);

    if let Ok(mut node) = q.q_architect.single_mut() {
        node.display = if is_architect {
            Display::Flex
        } else {
            Display::None
        };
    }

    if !is_architect {
        arch_category_state.0 = None;
    }

    if let Ok(mut node) = q.q_category_panel.single_mut() {
        node.display = if is_architect {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (mut node, panel) in q.q_building_panels.iter_mut() {
        node.display = if is_architect && arch_category_state.0 == Some(panel.0) {
            Display::Flex
        } else {
            Display::None
        };
    }

    if let Ok(mut node) = q.q_zones.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Zones) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q.q_orders.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Orders) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q.q_dream.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Dream) {
            Display::Flex
        } else {
            Display::None
        };
    }
}
