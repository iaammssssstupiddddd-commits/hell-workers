//! 情報パネル
//!
//! Startup時に常駐UIを生成し、選択状態に応じて差分更新する。

use crate::entities::damned_soul::Gender;
use crate::entities::damned_soul::{DamnedSoul, IdleState};
use crate::entities::familiar::Familiar;
use crate::interface::ui::components::{InfoPanel, UiInputBlocker, UiNodeRegistry, UiSlot};
use crate::interface::ui::presentation::{EntityInspectionModel, build_entity_inspection_model};
use crate::interface::ui::theme::UiTheme;
use crate::relationships::{CommandedBy, TaskWorkers};
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::FamiliarSpatialGrid;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient, RelativeCursorPosition};

#[derive(Resource, Default)]
pub struct InfoPanelState {
    last: Option<InfoPanelViewModel>,
}

#[derive(Clone, PartialEq)]
enum InfoPanelViewModel {
    Soul(SoulInfoViewModel),
    Simple(SimpleInfoViewModel),
}

#[derive(Clone, PartialEq)]
struct SoulInfoViewModel {
    header: String,
    gender: Option<Gender>,
    motivation: String,
    stress: String,
    fatigue: String,
    task: String,
    inventory: String,
    common: String,
}

#[derive(Clone, PartialEq)]
struct SimpleInfoViewModel {
    header: String,
    common: String,
}

pub fn spawn_info_panel_ui(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Px(theme.sizes.info_panel_width),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.spacing.panel_top),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
                display: Display::None,
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: 0.0,
                stops: vec![
                    ColorStop::new(theme.panels.info_panel.top, Val::Percent(0.0)),
                    ColorStop::new(theme.panels.info_panel.bottom, Val::Percent(100.0)),
                ],
                ..default()
            }),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            InfoPanel,
            UiSlot::InfoPanelRoot,
        ))
        .id();
    commands.entity(parent_entity).add_child(root);
    ui_nodes.set_slot(UiSlot::InfoPanelRoot, root);

    commands.entity(root).with_children(|parent| {
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            })
            .with_children(|row| {
                let header = row
                    .spawn((
                        Text::new(""),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_title,
                            ..default()
                        },
                        TextColor(theme.colors.text_accent),
                        UiSlot::Header,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::Header, header);

                let gender = row
                    .spawn((
                        ImageNode::default(),
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            margin: UiRect::left(Val::Px(8.0)),
                            display: Display::None,
                            ..default()
                        },
                        UiSlot::GenderIcon,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::GenderIcon, gender);
            });

        let stats = parent
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                UiSlot::InfoPanelStatsGroup,
            ))
            .with_children(|col| {
                let motivation = col
                    .spawn((
                        Text::new(""),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_small,
                            ..default()
                        },
                        UiSlot::StatMotivation,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::StatMotivation, motivation);

                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(game_assets.icon_stress.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            margin: UiRect::right(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                    let stress = row
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            UiSlot::StatStress,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::StatStress, stress);
                });

                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(game_assets.icon_fatigue.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            margin: UiRect::right(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                    let fatigue = row
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            UiSlot::StatFatigue,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::StatFatigue, fatigue);
                });

                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|row| {
                    let task = row
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            UiSlot::TaskText,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::TaskText, task);
                });

                let inventory = col
                    .spawn((
                        Text::new(""),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_small,
                            ..default()
                        },
                        UiSlot::InventoryText,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::InventoryText, inventory);
            })
            .id();
        ui_nodes.set_slot(UiSlot::InfoPanelStatsGroup, stats);

        let common = parent
            .spawn((
                Text::new(""),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_item,
                    ..default()
                },
                TextColor(theme.colors.text_primary),
                UiSlot::CommonText,
            ))
            .id();
        ui_nodes.set_slot(UiSlot::CommonText, common);
    });
}

fn set_text_slot(
    ui_nodes: &UiNodeRegistry,
    q_text: &mut Query<&mut Text>,
    slot: UiSlot,
    value: &str,
) {
    let Some(entity) = ui_nodes.get_slot(slot) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        if text.0 != value {
            text.0 = value.to_string();
        }
    }
}

fn set_display_slot(
    ui_nodes: &UiNodeRegistry,
    q_node: &mut Query<&mut Node>,
    slot: UiSlot,
    display: Display,
) {
    let Some(entity) = ui_nodes.get_slot(slot) else {
        return;
    };
    if let Ok(mut node) = q_node.get_mut(entity) {
        if node.display != display {
            node.display = display;
        }
    }
}

fn update_gender_icon(
    ui_nodes: &UiNodeRegistry,
    q_gender: &mut Query<&mut ImageNode>,
    q_node: &mut Query<&mut Node>,
    game_assets: &crate::assets::GameAssets,
    gender: Option<Gender>,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::GenderIcon) else {
        return;
    };
    if let Ok(mut icon) = q_gender.get_mut(entity) {
        if let Some(gender) = gender {
            set_display_slot(ui_nodes, q_node, UiSlot::GenderIcon, Display::Flex);
            icon.image = match gender {
                Gender::Male => game_assets.icon_male.clone(),
                Gender::Female => game_assets.icon_female.clone(),
            };
        } else {
            set_display_slot(ui_nodes, q_node, UiSlot::GenderIcon, Display::None);
        }
    }
}

fn to_view_model(model: EntityInspectionModel) -> InfoPanelViewModel {
    if let Some(soul) = model.soul {
        InfoPanelViewModel::Soul(SoulInfoViewModel {
            header: model.header,
            gender: soul.gender,
            motivation: soul.motivation,
            stress: soul.stress,
            fatigue: soul.fatigue,
            task: soul.task,
            inventory: soul.inventory,
            common: soul.common,
        })
    } else {
        InfoPanelViewModel::Simple(SimpleInfoViewModel {
            header: model.header,
            common: model.common_text,
        })
    }
}

#[derive(SystemParam)]
pub(crate) struct InspectionQueryParam<'w, 's> {
    q_souls: Query<
        'w,
        's,
        (
            &'static DamnedSoul,
            &'static AssignedTask,
            &'static Transform,
            &'static IdleState,
            Option<&'static CommandedBy>,
            Option<&'static crate::systems::logistics::Inventory>,
            Option<&'static crate::entities::damned_soul::SoulIdentity>,
        ),
    >,
    q_blueprints: Query<'w, 's, &'static Blueprint>,
    q_familiars: Query<
        'w,
        's,
        (
            &'static Familiar,
            &'static crate::entities::familiar::FamiliarOperation,
        ),
    >,
    q_familiars_escape: Query<'w, 's, (&'static Transform, &'static Familiar)>,
    familiar_grid: Res<'w, FamiliarSpatialGrid>,
    q_items: Query<'w, 's, &'static crate::systems::logistics::ResourceItem>,
    q_trees: Query<'w, 's, &'static crate::systems::jobs::Tree>,
    q_rocks: Query<'w, 's, &'static crate::systems::jobs::Rock>,
    q_designations: Query<
        'w,
        's,
        (
            &'static crate::systems::jobs::Designation,
            Option<&'static crate::systems::jobs::IssuedBy>,
            Option<&'static TaskWorkers>,
        ),
    >,
    q_buildings: Query<
        'w,
        's,
        (
            &'static crate::systems::jobs::Building,
            Option<&'static crate::systems::logistics::Stockpile>,
            Option<&'static crate::relationships::StoredItems>,
            Option<&'static crate::systems::jobs::MudMixerStorage>,
        ),
    >,
}

pub fn info_panel_system(
    game_assets: Res<crate::assets::GameAssets>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    ui_nodes: Res<UiNodeRegistry>,
    mut panel_state: ResMut<InfoPanelState>,
    mut q_text: Query<&mut Text>,
    mut q_node: Query<&mut Node>,
    mut q_gender: Query<&mut ImageNode>,
    inspection: InspectionQueryParam,
) {
    let next_model = selected.0.and_then(|entity| {
        build_entity_inspection_model(
            entity,
            &inspection.q_souls,
            &inspection.q_blueprints,
            &inspection.q_familiars,
            &inspection.q_familiars_escape,
            &inspection.familiar_grid,
            &inspection.q_items,
            &inspection.q_trees,
            &inspection.q_rocks,
            &inspection.q_designations,
            &inspection.q_buildings,
        )
        .map(to_view_model)
    });

    if panel_state.last == next_model {
        return;
    }

    set_display_slot(
        &ui_nodes,
        &mut q_node,
        UiSlot::InfoPanelRoot,
        if next_model.is_some() {
            Display::Flex
        } else {
            Display::None
        },
    );

    match &next_model {
        Some(InfoPanelViewModel::Soul(soul)) => {
            set_display_slot(
                &ui_nodes,
                &mut q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::Flex,
            );
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::Header, &soul.header);
            set_text_slot(
                &ui_nodes,
                &mut q_text,
                UiSlot::StatMotivation,
                &soul.motivation,
            );
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::StatStress, &soul.stress);
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::StatFatigue, &soul.fatigue);
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::TaskText, &soul.task);
            set_text_slot(
                &ui_nodes,
                &mut q_text,
                UiSlot::InventoryText,
                &soul.inventory,
            );
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::CommonText, &soul.common);
            update_gender_icon(
                &ui_nodes,
                &mut q_gender,
                &mut q_node,
                &game_assets,
                soul.gender,
            );
        }
        Some(InfoPanelViewModel::Simple(simple)) => {
            set_display_slot(
                &ui_nodes,
                &mut q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::None,
            );
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::Header, &simple.header);
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::CommonText, &simple.common);
            update_gender_icon(&ui_nodes, &mut q_gender, &mut q_node, &game_assets, None);
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::StatMotivation, "");
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::StatStress, "");
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::StatFatigue, "");
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::TaskText, "");
            set_text_slot(&ui_nodes, &mut q_text, UiSlot::InventoryText, "");
        }
        None => {
            set_display_slot(
                &ui_nodes,
                &mut q_node,
                UiSlot::InfoPanelStatsGroup,
                Display::None,
            );
            update_gender_icon(&ui_nodes, &mut q_gender, &mut q_node, &game_assets, None);
        }
    }

    panel_state.last = next_model;
}
