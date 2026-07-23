use super::model::{
    InfoPanelViewModel, next_stockpile_acceptance, next_stockpile_priority, to_view_model,
};
use super::state::{InfoPanelPinState, InfoPanelState};
use crate::components::{
    InfoPanelNodes, MenuAction, MenuButton, SoulRenameState, UiNodeRegistry, UiSlot,
};
use crate::intents::StockpilePolicyEditTarget;
use crate::models::inspection::{EntityInspectionViewModel, InspectionSoulGender};
use crate::selection::SelectedEntity;
use crate::setup::UiAssets;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_logistics::{StockpilePolicyPatch, StockpilePolicyState};

#[derive(SystemParam)]
pub struct InfoPanelRes<'w, A: UiAssets + Resource + 'static> {
    pub game_assets: Res<'w, A>,
    pub info_nodes: Res<'w, InfoPanelNodes>,
    pub ui_nodes: Res<'w, UiNodeRegistry>,
    pub inspection_view_model: Res<'w, EntityInspectionViewModel>,
}

#[derive(SystemParam)]
pub struct InfoPanelNodeQueries<'w, 's> {
    pub q_text: Query<'w, 's, &'static mut Text>,
    pub q_node: Query<'w, 's, &'static mut Node>,
    pub q_gender: Query<'w, 's, &'static mut ImageNode>,
    pub q_menu_button: Query<'w, 's, &'static mut MenuButton>,
}

fn entity_for_slot(
    info_nodes: &InfoPanelNodes,
    ui_nodes: &UiNodeRegistry,
    slot: UiSlot,
) -> Option<Entity> {
    let info_entity = match slot {
        UiSlot::InfoPanelRoot => info_nodes.root,
        UiSlot::InfoPanelStatsGroup => info_nodes.stats_group,
        UiSlot::InfoPanelUnpinButton => info_nodes.unpin_button,
        UiSlot::Header => info_nodes.header,
        UiSlot::GenderIcon => info_nodes.gender_icon,
        UiSlot::StatMotivation => info_nodes.motivation,
        UiSlot::StatStress => info_nodes.stress,
        UiSlot::StatFatigue => info_nodes.fatigue,
        UiSlot::StatDream => info_nodes.dream,
        UiSlot::TaskText => info_nodes.task,
        UiSlot::InventoryText => info_nodes.inventory,
        UiSlot::CommonText => info_nodes.common,
        _ => None,
    };
    info_entity.or_else(|| ui_nodes.get_slot(slot))
}

fn set_node_display(entity: Option<Entity>, q_node: &mut Query<&mut Node>, display: Display) {
    let Some(entity) = entity else {
        return;
    };
    if let Ok(mut node) = q_node.get_mut(entity) {
        node.display = display;
    }
}

fn set_text_entity(entity: Option<Entity>, q_text: &mut Query<&mut Text>, value: &str) {
    let Some(entity) = entity else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity)
        && text.0 != value
    {
        text.0 = value.to_string();
    }
}

fn set_menu_action(
    entity: Option<Entity>,
    q_menu_button: &mut Query<&mut MenuButton>,
    action: MenuAction,
) {
    let Some(entity) = entity else {
        return;
    };
    if let Ok(mut button) = q_menu_button.get_mut(entity) {
        button.0 = action;
    }
}

fn set_text_slot(
    info_nodes: &InfoPanelNodes,
    ui_nodes: &UiNodeRegistry,
    q_text: &mut Query<&mut Text>,
    slot: UiSlot,
    value: &str,
) {
    let Some(entity) = entity_for_slot(info_nodes, ui_nodes, slot) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity)
        && text.0 != value
    {
        text.0 = value.to_string();
    }
}

fn set_display_slot(
    info_nodes: &InfoPanelNodes,
    ui_nodes: &UiNodeRegistry,
    q_node: &mut Query<&mut Node>,
    slot: UiSlot,
    display: Display,
) {
    let Some(entity) = entity_for_slot(info_nodes, ui_nodes, slot) else {
        return;
    };
    if let Ok(mut node) = q_node.get_mut(entity)
        && node.display != display
    {
        node.display = display;
    }
}

fn update_gender_icon<A: UiAssets>(
    info_nodes: &InfoPanelNodes,
    ui_nodes: &UiNodeRegistry,
    q_gender: &mut Query<&mut ImageNode>,
    q_node: &mut Query<&mut Node>,
    game_assets: &A,
    gender: Option<InspectionSoulGender>,
) {
    let Some(entity) = entity_for_slot(info_nodes, ui_nodes, UiSlot::GenderIcon) else {
        return;
    };
    if let Ok(mut icon) = q_gender.get_mut(entity) {
        if let Some(gender) = gender {
            set_display_slot(
                info_nodes,
                ui_nodes,
                q_node,
                UiSlot::GenderIcon,
                Display::Flex,
            );
            icon.image = match gender {
                InspectionSoulGender::Male => game_assets.icon_male().clone(),
                InspectionSoulGender::Female => game_assets.icon_female().clone(),
            };
        } else {
            set_display_slot(
                info_nodes,
                ui_nodes,
                q_node,
                UiSlot::GenderIcon,
                Display::None,
            );
        }
    }
}

pub fn info_panel_system<A: UiAssets + Resource>(
    res: InfoPanelRes<A>,
    _selected: Res<SelectedEntity>,
    pin_state: ResMut<InfoPanelPinState>,
    mut panel_state: ResMut<InfoPanelState>,
    rename_state: Res<SoulRenameState>,
    mut queries: InfoPanelNodeQueries,
) {
    let next_model = res.inspection_view_model.model.clone().map(to_view_model);

    let pinned = pin_state.entity.is_some();
    let rename_target = match &next_model {
        Some(InfoPanelViewModel::Soul(soul))
            if rename_state
                .active
                .is_some_and(|active| active.target == soul.entity) =>
        {
            Some(soul.entity)
        }
        _ => None,
    };

    if panel_state.last == next_model
        && panel_state.last_pinned == pinned
        && panel_state.last_rename_target == rename_target
    {
        return;
    }

    set_display_slot(
        &res.info_nodes,
        &res.ui_nodes,
        &mut queries.q_node,
        UiSlot::InfoPanelRoot,
        if next_model.is_some() {
            Display::Flex
        } else {
            Display::None
        },
    );
    set_display_slot(
        &res.info_nodes,
        &res.ui_nodes,
        &mut queries.q_node,
        UiSlot::InfoPanelUnpinButton,
        if pinned { Display::Flex } else { Display::None },
    );

    match &next_model {
        Some(InfoPanelViewModel::Soul(soul)) => {
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::None,
            );
            let renaming = rename_state
                .active
                .is_some_and(|active| active.target == soul.entity);
            set_node_display(
                res.info_nodes.rename_button,
                &mut queries.q_node,
                Display::Flex,
            );
            set_node_display(
                res.info_nodes.rename_field_container,
                &mut queries.q_node,
                if renaming {
                    Display::Flex
                } else {
                    Display::None
                },
            );
            set_display_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::Flex,
            );
            set_display_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_node,
                UiSlot::Header,
                if renaming {
                    Display::None
                } else {
                    Display::Flex
                },
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::Header,
                &soul.header,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatMotivation,
                &soul.motivation,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatStress,
                &soul.stress,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatFatigue,
                &soul.fatigue,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatDream,
                &soul.dream,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::TaskText,
                &soul.task,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::InventoryText,
                &soul.inventory,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::CommonText,
                &soul.common,
            );
            update_gender_icon(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_gender,
                &mut queries.q_node,
                &*res.game_assets,
                soul.gender,
            );
        }
        Some(InfoPanelViewModel::Stockpile(stockpile)) => {
            set_node_display(
                res.info_nodes.rename_button,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.rename_field_container,
                &mut queries.q_node,
                Display::None,
            );
            set_display_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::None,
            );
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::Flex,
            );
            set_display_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_node,
                UiSlot::Header,
                Display::Flex,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::Header,
                &stockpile.header,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::CommonText,
                &stockpile.common,
            );
            update_gender_icon(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_gender,
                &mut queries.q_node,
                &*res.game_assets,
                None,
            );

            let state_label = match stockpile.state {
                StockpilePolicyState::Accepting => "Accepting",
                StockpilePolicyState::TargetReached => "Target Reached",
                StockpilePolicyState::Draining => "Draining",
            };
            let resource_label = stockpile
                .current_resource
                .map(|resource| format!("{resource:?}"))
                .unwrap_or_else(|| "Empty".to_string());
            set_text_entity(
                res.info_nodes.stockpile_state,
                &mut queries.q_text,
                &format!("State: {state_label}"),
            );
            set_text_entity(
                res.info_nodes.stockpile_current,
                &mut queries.q_text,
                &format!(
                    "Stored: {}/{} ({resource_label}) | Incoming: {}",
                    stockpile.current_amount, stockpile.capacity, stockpile.incoming_amount
                ),
            );
            set_text_entity(
                res.info_nodes.stockpile_acceptance_text,
                &mut queries.q_text,
                &format!("Acceptance: {:?} (cycle)", stockpile.acceptance),
            );
            set_text_entity(
                res.info_nodes.stockpile_target_text,
                &mut queries.q_text,
                &format!("Target: {}/{}", stockpile.target_amount, stockpile.capacity),
            );
            set_text_entity(
                res.info_nodes.stockpile_priority_text,
                &mut queries.q_text,
                &format!("Inbound Priority: {:?} (cycle)", stockpile.inbound_priority),
            );
            let export_label =
                if stockpile.state == StockpilePolicyState::Draining && !stockpile.allow_export {
                    "Export: Off (Draining override)"
                } else if stockpile.allow_export {
                    "Export: On"
                } else {
                    "Export: Off"
                };
            set_text_entity(
                res.info_nodes.stockpile_export_text,
                &mut queries.q_text,
                export_label,
            );

            let single = StockpilePolicyEditTarget::Single(stockpile.entity);
            set_menu_action(
                res.info_nodes.stockpile_acceptance_button,
                &mut queries.q_menu_button,
                MenuAction::ApplyStockpilePolicy {
                    target: single,
                    patch: StockpilePolicyPatch {
                        acceptance: Some(next_stockpile_acceptance(stockpile.acceptance)),
                        ..default()
                    },
                },
            );
            set_menu_action(
                res.info_nodes.stockpile_target_decrease_button,
                &mut queries.q_menu_button,
                MenuAction::ApplyStockpilePolicy {
                    target: single,
                    patch: StockpilePolicyPatch {
                        target_amount: Some(stockpile.target_amount.saturating_sub(1)),
                        ..default()
                    },
                },
            );
            set_menu_action(
                res.info_nodes.stockpile_target_increase_button,
                &mut queries.q_menu_button,
                MenuAction::ApplyStockpilePolicy {
                    target: single,
                    patch: StockpilePolicyPatch {
                        target_amount: Some(
                            stockpile
                                .target_amount
                                .saturating_add(1)
                                .min(stockpile.capacity),
                        ),
                        ..default()
                    },
                },
            );
            set_menu_action(
                res.info_nodes.stockpile_priority_button,
                &mut queries.q_menu_button,
                MenuAction::ApplyStockpilePolicy {
                    target: single,
                    patch: StockpilePolicyPatch {
                        inbound_priority: Some(next_stockpile_priority(stockpile.inbound_priority)),
                        ..default()
                    },
                },
            );
            set_menu_action(
                res.info_nodes.stockpile_export_button,
                &mut queries.q_menu_button,
                MenuAction::ApplyStockpilePolicy {
                    target: single,
                    patch: StockpilePolicyPatch {
                        allow_export: Some(!stockpile.allow_export),
                        ..default()
                    },
                },
            );
            set_menu_action(
                res.info_nodes.stockpile_area_button,
                &mut queries.q_menu_button,
                MenuAction::BeginStockpilePolicyRangeEdit {
                    patch: StockpilePolicyPatch {
                        acceptance: Some(stockpile.acceptance),
                        inbound_priority: Some(stockpile.inbound_priority),
                        target_amount: Some(stockpile.target_amount),
                        allow_export: Some(stockpile.allow_export),
                    },
                },
            );
        }
        Some(InfoPanelViewModel::Simple(simple)) => {
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.rename_button,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.rename_field_container,
                &mut queries.q_node,
                Display::None,
            );
            set_display_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::None,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::Header,
                &simple.header,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::CommonText,
                &simple.common,
            );
            update_gender_icon(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_gender,
                &mut queries.q_node,
                &*res.game_assets,
                None,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatMotivation,
                "",
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatStress,
                "",
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatFatigue,
                "",
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::StatDream,
                "",
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::TaskText,
                "",
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::InventoryText,
                "",
            );
        }
        None => {
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.rename_button,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.rename_field_container,
                &mut queries.q_node,
                Display::None,
            );
            set_display_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::None,
            );
            update_gender_icon(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_gender,
                &mut queries.q_node,
                &*res.game_assets,
                None,
            );
        }
    }

    panel_state.last = next_model;
    panel_state.last_pinned = pinned;
    panel_state.last_rename_target = rename_target;
}
