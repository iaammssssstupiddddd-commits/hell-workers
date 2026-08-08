use super::model::{
    InfoPanelViewModel, next_power_priority, next_stockpile_priority,
    stockpile_acceptance_row_label, stockpile_acceptance_summary, to_view_model,
};
use super::state::{InfoPanelPinState, InfoPanelState};
use crate::components::{
    InfoPanelNodes, MenuAction, MenuButton, SoulRenameState, UiNodeRegistry, UiSlot,
};
use crate::intents::StockpilePolicyEditTarget;
use crate::models::inspection::{
    EntityInspectionViewModel, InspectionSoulGender, PowerInspectionFields,
};
use crate::power::{
    PowerAllocationModeValue, PowerInspectionRole, PowerShedReasonValue, PowerSupplyStateValue,
};
use crate::selection::SelectedEntity;
use crate::setup::UiAssets;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_logistics::{StockpileAcceptance, StockpilePolicyPatch, StockpilePolicyState};

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

fn power_supply_label(fields: &PowerInspectionFields) -> &'static str {
    if fields.role == PowerInspectionRole::Generator {
        return if fields.grid.is_none() {
            "Supply: Generator / disconnected"
        } else if fields.allocation_mode.is_none() {
            "Supply: Generator / grid rebuilding"
        } else {
            "Supply: Generator"
        };
    }

    match fields.supply_state {
        Some(PowerSupplyStateValue::Supplied) => "Supply: Supplied",
        Some(PowerSupplyStateValue::Shed {
            reason: PowerShedReasonValue::InsufficientGeneration,
        }) => "Supply: Shed — insufficient generation",
        Some(PowerSupplyStateValue::Shed {
            reason: PowerShedReasonValue::RestoreMargin,
        }) => "Supply: Shed — waiting for restore margin",
        Some(PowerSupplyStateValue::Shed {
            reason: PowerShedReasonValue::LegacyGlobalDeficit,
        }) => "Supply: Shed — legacy grid deficit",
        Some(PowerSupplyStateValue::Disconnected) => "Supply: Disconnected",
        Some(PowerSupplyStateValue::InvalidDemand) => "Supply: Invalid demand",
        None => "Supply: Rebuilding",
    }
}

fn power_mode_label(mode: PowerAllocationModeValue) -> &'static str {
    match mode {
        PowerAllocationModeValue::PriorityPrefix => "Priority prefix",
        PowerAllocationModeValue::LegacyAllOrNone => "Legacy all-or-none",
    }
}

fn soul_spa_status_label(soul_spa: &super::model::SoulSpaInfoViewModel) -> String {
    if !soul_spa.operational {
        format!(
            "Status: Constructing ({}/{} bones)",
            soul_spa.bones_delivered, soul_spa.bones_required
        )
    } else if soul_spa.occupied_slots > soul_spa.active_slots {
        format!(
            "Draining ({} active / {} configured)",
            soul_spa.occupied_slots, soul_spa.active_slots
        )
    } else {
        format!(
            "Operational ({} active / {} configured)",
            soul_spa.occupied_slots, soul_spa.active_slots
        )
    }
}

fn update_power_section(
    target: Entity,
    fields: Option<&PowerInspectionFields>,
    nodes: &InfoPanelNodes,
    q_text: &mut Query<&mut Text>,
    q_node: &mut Query<&mut Node>,
    q_menu_button: &mut Query<&mut MenuButton>,
) {
    set_node_display(
        nodes.power_group,
        q_node,
        if fields.is_some() {
            Display::Flex
        } else {
            Display::None
        },
    );
    let Some(fields) = fields else {
        return;
    };

    set_text_entity(
        nodes.power_connection,
        q_text,
        if fields.grid.is_some() {
            "Connection: Connected"
        } else {
            "Connection: Disconnected"
        },
    );
    let mut flow = match (
        fields.generation_watts,
        fields.total_demand_watts,
        fields.served_demand_watts,
        fields.allocation_mode,
    ) {
        (Some(generation), Some(total), Some(served), Some(mode)) => format!(
            "Grid: {generation:.1}W generated / {served:.1}W of {total:.1}W served [{}]",
            power_mode_label(mode)
        ),
        (Some(generation), Some(total), _, _) => {
            format!("Grid: {generation:.1}W generated / {total:.1}W demand (rebuilding)")
        }
        _ => "Grid: unavailable".to_string(),
    };
    match (fields.reserve_watts, fields.deficit_watts) {
        (Some(_reserve), Some(deficit)) if deficit > 0.0 => {
            flow.push_str(&format!("\nBalance: {deficit:.1}W deficit"));
        }
        (Some(reserve), Some(_)) => {
            flow.push_str(&format!("\nBalance: {reserve:.1}W reserve"));
        }
        _ => {}
    }
    if let (Some(consumers), Some(supplied), Some(shed), Some(invalid)) = (
        fields.consumer_count,
        fields.supplied_count,
        fields.shed_count,
        fields.invalid_count,
    ) {
        flow.push_str(&format!(
            "\nConsumers: {supplied}/{consumers} supplied, {shed} shed, {invalid} invalid"
        ));
        let shed_order = if fields.shed_order_labels.is_empty() {
            "none".to_string()
        } else {
            fields.shed_order_labels.join(" → ")
        };
        flow.push_str(&format!("\nShed order: {shed_order}"));
    }
    set_text_entity(nodes.power_flow, q_text, &flow);
    let mut state = power_supply_label(fields).to_string();
    if let Some(demand) = fields.demand_watts {
        state.push_str(&format!(" / Demand: {demand:.1}W"));
    }
    set_text_entity(nodes.power_state, q_text, &state);

    set_node_display(
        nodes.power_priority_button,
        q_node,
        if fields.priority.is_some() {
            Display::Flex
        } else {
            Display::None
        },
    );
    if let Some(priority) = fields.priority {
        let next = next_power_priority(priority);
        set_text_entity(
            nodes.power_priority_text,
            q_text,
            &format!("Priority: {priority:?} → {next:?}"),
        );
        set_menu_action(
            nodes.power_priority_button,
            q_menu_button,
            MenuAction::SetPowerConsumerPriority {
                target,
                priority: next,
            },
        );
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

    let (power_target, power_fields) = match &next_model {
        Some(InfoPanelViewModel::SoulSpa(soul_spa)) => (soul_spa.entity, soul_spa.power.as_ref()),
        Some(InfoPanelViewModel::Power(power)) => (power.entity, Some(&power.fields)),
        _ => (Entity::PLACEHOLDER, None),
    };
    update_power_section(
        power_target,
        power_fields,
        &res.info_nodes,
        &mut queries.q_text,
        &mut queries.q_node,
        &mut queries.q_menu_button,
    );

    match &next_model {
        Some(InfoPanelViewModel::Soul(soul)) => {
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.soul_spa_group,
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
                res.info_nodes.soul_spa_group,
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
                StockpilePolicyState::Disabled => "Disabled",
            };
            let resource_label = stockpile
                .current_resource
                .map(|resource| resource.display_name().to_string())
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
                res.info_nodes.stockpile_acceptance_summary,
                &mut queries.q_text,
                &stockpile_acceptance_summary(stockpile.acceptance),
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
                res.info_nodes.stockpile_acceptance_all_button,
                &mut queries.q_menu_button,
                MenuAction::ApplyStockpilePolicy {
                    target: single,
                    patch: StockpilePolicyPatch {
                        acceptance: Some(StockpileAcceptance::Any),
                        ..default()
                    },
                },
            );
            set_menu_action(
                res.info_nodes.stockpile_acceptance_none_button,
                &mut queries.q_menu_button,
                MenuAction::ApplyStockpilePolicy {
                    target: single,
                    patch: StockpilePolicyPatch {
                        acceptance: Some(StockpileAcceptance::none()),
                        ..default()
                    },
                },
            );
            for row in &res.info_nodes.stockpile_acceptance_rows {
                set_text_entity(
                    Some(row.text),
                    &mut queries.q_text,
                    &stockpile_acceptance_row_label(stockpile.acceptance, row.resource_type),
                );
                set_menu_action(
                    Some(row.button),
                    &mut queries.q_menu_button,
                    MenuAction::ApplyStockpilePolicy {
                        target: single,
                        patch: StockpilePolicyPatch {
                            acceptance: Some(stockpile.acceptance.with_resource(
                                row.resource_type,
                                !stockpile.acceptance.accepts(row.resource_type),
                            )),
                            ..default()
                        },
                    },
                );
            }
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
        Some(InfoPanelViewModel::SoulSpa(soul_spa)) => {
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.soul_spa_group,
                &mut queries.q_node,
                Display::Flex,
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
                &soul_spa.header,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::CommonText,
                &soul_spa.common,
            );
            update_gender_icon(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_gender,
                &mut queries.q_node,
                &*res.game_assets,
                None,
            );

            let status = soul_spa_status_label(soul_spa);
            set_text_entity(res.info_nodes.soul_spa_status, &mut queries.q_text, &status);
            set_node_display(
                res.info_nodes.soul_spa_output,
                &mut queries.q_node,
                if soul_spa.operational {
                    Display::Flex
                } else {
                    Display::None
                },
            );
            set_text_entity(
                res.info_nodes.soul_spa_output,
                &mut queries.q_text,
                &format!("Output: {:.1}W", soul_spa.output_watts),
            );
            set_text_entity(
                res.info_nodes.soul_spa_slots_text,
                &mut queries.q_text,
                &format!(
                    "Active slots: {}/{}",
                    soul_spa.active_slots, soul_spa.max_active_slots
                ),
            );
            set_node_display(
                res.info_nodes.soul_spa_controls,
                &mut queries.q_node,
                if soul_spa.operational {
                    Display::Flex
                } else {
                    Display::None
                },
            );
            set_menu_action(
                res.info_nodes.soul_spa_slots_decrease_button,
                &mut queries.q_menu_button,
                MenuAction::SetSoulSpaActiveSlots {
                    target: soul_spa.entity,
                    active_slots: soul_spa.active_slots.saturating_sub(1),
                },
            );
            set_menu_action(
                res.info_nodes.soul_spa_slots_increase_button,
                &mut queries.q_menu_button,
                MenuAction::SetSoulSpaActiveSlots {
                    target: soul_spa.entity,
                    active_slots: soul_spa
                        .active_slots
                        .saturating_add(1)
                        .min(soul_spa.max_active_slots),
                },
            );
            set_node_display(
                res.info_nodes.soul_spa_cancel_button,
                &mut queries.q_node,
                if soul_spa.operational {
                    Display::None
                } else {
                    Display::Flex
                },
            );
            set_menu_action(
                res.info_nodes.soul_spa_cancel_button,
                &mut queries.q_menu_button,
                MenuAction::CancelSoulSpaConstruction {
                    target: soul_spa.entity,
                },
            );
        }
        Some(InfoPanelViewModel::Power(power)) => {
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.soul_spa_group,
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
                &power.header,
            );
            set_text_slot(
                &res.info_nodes,
                &res.ui_nodes,
                &mut queries.q_text,
                UiSlot::CommonText,
                &power.common,
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
        Some(InfoPanelViewModel::Simple(simple)) => {
            set_node_display(
                res.info_nodes.stockpile_group,
                &mut queries.q_node,
                Display::None,
            );
            set_node_display(
                res.info_nodes.soul_spa_group,
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
                res.info_nodes.soul_spa_group,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructing_soul_spa_status_includes_material_progress() {
        let model = super::super::model::SoulSpaInfoViewModel {
            entity: Entity::PLACEHOLDER,
            header: "Soul Spa".to_string(),
            operational: false,
            bones_delivered: 7,
            bones_required: 20,
            occupied_slots: 0,
            active_slots: 4,
            max_active_slots: 4,
            output_watts: 0.0,
            power: None,
            common: String::new(),
        };

        assert_eq!(
            soul_spa_status_label(&model),
            "Status: Constructing (7/20 bones)"
        );
    }

    #[test]
    fn stable_generator_is_not_labeled_as_rebuilding() {
        let fields = PowerInspectionFields {
            role: PowerInspectionRole::Generator,
            grid: Some(Entity::PLACEHOLDER),
            allocation_mode: Some(PowerAllocationModeValue::PriorityPrefix),
            generation_watts: Some(2.0),
            total_demand_watts: Some(1.0),
            served_demand_watts: Some(1.0),
            reserve_watts: Some(1.0),
            deficit_watts: Some(0.0),
            consumer_count: Some(1),
            supplied_count: Some(1),
            shed_count: Some(0),
            invalid_count: Some(0),
            shed_order_labels: Vec::new(),
            demand_watts: None,
            priority: None,
            supply_state: None,
        };

        assert_eq!(power_supply_label(&fields), "Supply: Generator");
    }
}
