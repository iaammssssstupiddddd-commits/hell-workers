use super::model::{InfoPanelViewModel, to_view_model};
use super::state::{InfoPanelPinState, InfoPanelState};
use crate::entities::damned_soul::Gender;
use crate::interface::ui::components::{InfoPanelNodes, UiNodeRegistry, UiSlot};
use crate::interface::ui::presentation::EntityInspectionQuery;
use bevy::prelude::*;

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
        UiSlot::TaskText => info_nodes.task,
        UiSlot::InventoryText => info_nodes.inventory,
        UiSlot::CommonText => info_nodes.common,
        _ => None,
    };
    info_entity.or_else(|| ui_nodes.get_slot(slot))
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

fn update_gender_icon(
    info_nodes: &InfoPanelNodes,
    ui_nodes: &UiNodeRegistry,
    q_gender: &mut Query<&mut ImageNode>,
    q_node: &mut Query<&mut Node>,
    game_assets: &crate::assets::GameAssets,
    gender: Option<Gender>,
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
                Gender::Male => game_assets.icon_male.clone(),
                Gender::Female => game_assets.icon_female.clone(),
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

pub fn info_panel_system(
    game_assets: Res<crate::assets::GameAssets>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut pin_state: ResMut<InfoPanelPinState>,
    info_nodes: Res<InfoPanelNodes>,
    ui_nodes: Res<UiNodeRegistry>,
    mut panel_state: ResMut<InfoPanelState>,
    mut q_text: Query<&mut Text>,
    mut q_node: Query<&mut Node>,
    mut q_gender: Query<&mut ImageNode>,
    inspection: EntityInspectionQuery,
) {
    let mut inspected_entity = pin_state.entity.or(selected.0);
    let mut next_model =
        inspected_entity.and_then(|entity| inspection.build_model(entity).map(to_view_model));

    if pin_state.entity.is_some() && next_model.is_none() {
        pin_state.entity = None;
        inspected_entity = selected.0;
        next_model =
            inspected_entity.and_then(|entity| inspection.build_model(entity).map(to_view_model));
    }

    let pinned = pin_state.entity.is_some();
    if panel_state.last == next_model && panel_state.last_pinned == pinned {
        return;
    }

    set_display_slot(
        &info_nodes,
        &ui_nodes,
        &mut q_node,
        UiSlot::InfoPanelRoot,
        if next_model.is_some() {
            Display::Flex
        } else {
            Display::None
        },
    );
    set_display_slot(
        &info_nodes,
        &ui_nodes,
        &mut q_node,
        UiSlot::InfoPanelUnpinButton,
        if pinned { Display::Flex } else { Display::None },
    );

    match &next_model {
        Some(InfoPanelViewModel::Soul(soul)) => {
            set_display_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::Flex,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::Header,
                &soul.header,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::StatMotivation,
                &soul.motivation,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::StatStress,
                &soul.stress,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::StatFatigue,
                &soul.fatigue,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::TaskText,
                &soul.task,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::InventoryText,
                &soul.inventory,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::CommonText,
                &soul.common,
            );
            update_gender_icon(
                &info_nodes,
                &ui_nodes,
                &mut q_gender,
                &mut q_node,
                &game_assets,
                soul.gender,
            );
        }
        Some(InfoPanelViewModel::Simple(simple)) => {
            set_display_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::None,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::Header,
                &simple.header,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::CommonText,
                &simple.common,
            );
            update_gender_icon(
                &info_nodes,
                &ui_nodes,
                &mut q_gender,
                &mut q_node,
                &game_assets,
                None,
            );
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::StatMotivation,
                "",
            );
            set_text_slot(&info_nodes, &ui_nodes, &mut q_text, UiSlot::StatStress, "");
            set_text_slot(&info_nodes, &ui_nodes, &mut q_text, UiSlot::StatFatigue, "");
            set_text_slot(&info_nodes, &ui_nodes, &mut q_text, UiSlot::TaskText, "");
            set_text_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_text,
                UiSlot::InventoryText,
                "",
            );
        }
        None => {
            set_display_slot(
                &info_nodes,
                &ui_nodes,
                &mut q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::None,
            );
            update_gender_icon(
                &info_nodes,
                &ui_nodes,
                &mut q_gender,
                &mut q_node,
                &game_assets,
                None,
            );
        }
    }

    panel_state.last = next_model;
    panel_state.last_pinned = pinned;
}
