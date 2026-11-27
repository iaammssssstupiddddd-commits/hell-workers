use crate::components::{MenuButton, UiInputBlocker};
use crate::{
    UiIntent,
    setup::UiAssets,
    theme::{UiTheme, font_size_rem},
};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::ScrollArea;

#[derive(Component)]
pub struct ContractBody;
#[derive(Component)]
pub enum ContractDescription {
    Card,
    Todo,
}
#[derive(Component)]
pub struct ContractControl;
#[derive(Component)]
pub struct ContractTitle(pub bool);
#[derive(Component)]
pub struct PanelDetailReport;
#[derive(Component)]
pub struct PanelDetailStatus;

pub const REPORT_PANEL: UiIntent = UiIntent::ReportReceivingPanel {
    target: Entity::PLACEHOLDER,
    designation: 0,
    epoch: 0,
};

pub(crate) fn spawn_panel_report(
    parent: &mut ChildSpawnerCommands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    parent.spawn((
        PanelDetailStatus,
        Text::new(""),
        TextFont {
            font: assets.font_ui().clone().into(),
            font_size: font_size_rem(theme.typography.font_size_sm),
            ..default()
        },
        TextColor(theme.colors.text_primary_semantic),
        Node {
            display: Display::None,
            ..default()
        },
    ));
    parent
        .spawn((
            PanelDetailReport,
            Button,
            MenuButton(REPORT_PANEL),
            BackgroundColor(theme.colors.button_default),
            Node {
                display: Display::None,
                padding: UiRect::all(Val::Px(6.)),
                ..default()
            },
        ))
        .with_child((
            Text::new("受電盤の準備完了を連絡"),
            TextFont {
                font: assets.font_ui().clone().into(),
                font_size: font_size_rem(theme.typography.font_size_sm),
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
        ));
}

pub(crate) fn spawn(parent: &mut ChildSpawnerCommands, assets: &dyn UiAssets, theme: &UiTheme) {
    let font = TextFont {
        font: assets.font_ui().clone().into(),
        font_size: font_size_rem(theme.typography.font_size_sm),
        ..default()
    };
    parent
        .spawn((
            ContractBody,
            ScrollArea,
            UiInputBlocker,
            RelativeCursorPosition::default(),
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                row_gap: Val::Px(7.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
        ))
        .with_children(|card| {
            card.spawn((
                ContractTitle(false),
                Text::new("第一工区の給水所"),
                font.clone(),
                TextColor(theme.colors.text_accent_semantic),
            ));
            card.spawn((
                ContractDescription::Card,
                Text::new(""),
                font.clone(),
                TextColor(theme.colors.text_primary_semantic),
            ));
            for (label, action) in [
                (
                    "川辺の夜普請を開く",
                    UiIntent::SelectReceptionContract(true),
                ),
                (
                    "第一工区の給水所を開く",
                    UiIntent::SelectReceptionContract(false),
                ),
                ("A点へ", UiIntent::LocateReceivingPanel),
                ("夜普請を受注する", UiIntent::AcceptReception),
                ("指定盤を施工", UiIntent::PlaceReceivingPanel),
                ("準備完了を連絡", REPORT_PANEL),
                ("工区を表示／隠す", UiIntent::ToggleContractDistrict),
                ("受注する", UiIntent::AcceptWaterworks),
                ("契約用タンクを配置", UiIntent::PlaceWaterworks),
                ("仕事を確認", UiIntent::ViewWaterworksTasks),
                ("報酬を受け取る", UiIntent::ClaimWaterworks),
            ] {
                let mut button = card.spawn((
                    MenuButton(action),
                    Button,
                    ContractControl,
                    BackgroundColor(theme.colors.button_default),
                    Node {
                        min_height: Val::Px(24.0),
                        flex_shrink: 0.0,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        ..default()
                    },
                ));
                button.with_children(|b| {
                    b.spawn((
                        Text::new(label),
                        font.clone(),
                        TextColor(theme.colors.text_primary_semantic),
                    ));
                });
            }
        });
}

#[derive(Component)]
pub struct ContractTodoPanel;

pub(crate) fn spawn_todo(
    parent: &mut ChildSpawnerCommands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    use bevy::{picking::Pickable, ui::FocusPolicy};
    let font = TextFont {
        font: assets.font_ui().clone().into(),
        font_size: font_size_rem(theme.typography.font_size_sm),
        ..default()
    };
    parent
        .spawn((
            ContractTodoPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(theme.spacing.panel_top),
                right: Val::Px(theme.spacing.panel_margin_x),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                max_width: Val::Percent(45.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::left(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(theme.colors.bg_overlay),
            BorderColor::all(theme.colors.text_accent_semantic),
            Pickable::IGNORE,
            FocusPolicy::Pass,
        ))
        .with_children(|panel| {
            panel.spawn((
                ContractTitle(true),
                Text::new("契約ToDo　第一工区の給水所"),
                font.clone(),
                TextColor(theme.colors.text_accent_semantic),
                Pickable::IGNORE,
                FocusPolicy::Pass,
            ));
            panel.spawn((
                ContractDescription::Todo,
                Text::new(""),
                font,
                TextColor(theme.colors.text_primary_semantic),
                Pickable::IGNORE,
                FocusPolicy::Pass,
            ));
        });
}
