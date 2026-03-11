//! UIセットアップモジュール
//!
//! UIの初期構造を構築します。

mod bottom_bar;
mod dialogs;
mod entity_list;
mod panels;
mod submenus;
mod time_control;

use crate::components::{InfoPanelNodes, UiMountSlot, UiNodeRegistry, UiRoot, UiSlot};
use crate::theme::UiTheme;
use bevy::prelude::*;

pub trait UiAssets {
    fn font_ui(&self) -> &Handle<Font>;
    fn font_familiar(&self) -> &Handle<Font>;
    fn icon_arrow_down(&self) -> &Handle<Image>;
    fn glow_circle(&self) -> &Handle<Image>;
    fn icon_stress(&self) -> &Handle<Image>;
    fn icon_fatigue(&self) -> &Handle<Image>;
    fn icon_male(&self) -> &Handle<Image>;
    fn icon_female(&self) -> &Handle<Image>;
}

fn spawn_fps_display(
    commands: &mut Commands,
    theme: &UiTheme,
    parent: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let root = commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            left: Val::Px(theme.sizes.fps_left),
            top: Val::Px(theme.sizes.fps_top),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Start,
            ..default()
        },))
        .id();
    commands.entity(parent).add_child(root);

    commands.entity(root).with_children(|parent| {
        let text_entity = parent
            .spawn((
                Text::new("FPS: --"),
                TextFont {
                    font_size: theme.typography.font_size_title,
                    ..default()
                },
                TextColor(theme.colors.text_primary),
                UiSlot::FpsText,
            ))
            .id();
        ui_nodes.set_slot(UiSlot::FpsText, text_entity);
    });
}

fn spawn_area_edit_preview(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let preview = commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(theme.colors.tooltip_bg),
            BorderColor::all(theme.colors.tooltip_border),
            Text::new(""),
            TextFont {
                font: game_assets.font_ui().clone(),
                font_size: theme.typography.font_size_sm,
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
            ZIndex(40),
            UiSlot::AreaEditPreview,
            Name::new("Area Edit Preview"),
        ))
        .id();
    commands.entity(parent).add_child(preview);
    ui_nodes.set_slot(UiSlot::AreaEditPreview, preview);
}

fn spawn_ui_root(
    commands: &mut Commands,
) -> (Entity, Entity, Entity, Entity, Entity, Entity, Entity) {
    let ui_root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiRoot,
        ))
        .id();

    // 夢の泡専用レイヤー（最初の子 = パネル系より後ろに描画される）
    let dream_bubble_layer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiMountSlot::DreamBubbleLayer,
            Name::new("Dream Bubble Layer"),
        ))
        .id();

    let left = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiMountSlot::LeftPanel,
        ))
        .id();
    let right = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiMountSlot::RightPanel,
        ))
        .id();
    let bottom = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiMountSlot::Bottom,
        ))
        .id();
    let overlay = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiMountSlot::Overlay,
        ))
        .id();
    let top_right = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiMountSlot::TopRight,
        ))
        .id();
    let top_left = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            UiMountSlot::TopLeft,
        ))
        .id();

    // dream_bubble_layer を最初に追加することで、後続のパネル系より背後に描画される
    commands.entity(ui_root).add_children(&[
        dream_bubble_layer,
        left,
        right,
        bottom,
        top_right,
        top_left,
        overlay,
    ]);
    (
        ui_root,
        left,
        right,
        bottom,
        overlay,
        top_right,
        dream_bubble_layer,
    )
}

pub fn setup_ui<F, G>(
    mut commands: Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    mut ui_nodes: ResMut<UiNodeRegistry>,
    mut info_panel_nodes: ResMut<InfoPanelNodes>,
    spawn_root_panels: F,
    spawn_root_vignette: G,
) where
    F: FnOnce(&mut Commands, Entity, Entity, &mut UiNodeRegistry, &mut InfoPanelNodes),
    G: FnOnce(&mut Commands, Entity),
{
    let (_, left_slot, right_slot, bottom_slot, overlay_slot, top_right_slot, _dream_bubble_slot) =
        spawn_ui_root(&mut commands);

    bottom_bar::spawn_bottom_bar(
        &mut commands,
        game_assets,
        theme,
        bottom_slot,
        &mut ui_nodes,
    );
    submenus::spawn_submenus(&mut commands, game_assets, theme, bottom_slot);
    panels::spawn_panels(
        &mut commands,
        game_assets,
        theme,
        overlay_slot,
        &mut ui_nodes,
    );
    entity_list::spawn_entity_list_panel(&mut commands, game_assets, theme, left_slot);
    time_control::spawn_time_control(
        &mut commands,
        game_assets,
        theme,
        top_right_slot,
        &mut ui_nodes,
    );
    spawn_area_edit_preview(
        &mut commands,
        game_assets,
        theme,
        overlay_slot,
        &mut ui_nodes,
    );
    spawn_fps_display(&mut commands, theme, top_right_slot, &mut ui_nodes);
    dialogs::spawn_dialogs(
        &mut commands,
        game_assets,
        theme,
        overlay_slot,
        &mut ui_nodes,
    );
    spawn_root_panels(
        &mut commands,
        right_slot,
        overlay_slot,
        &mut ui_nodes,
        &mut info_panel_nodes,
    );
    spawn_root_vignette(&mut commands, overlay_slot);
}
