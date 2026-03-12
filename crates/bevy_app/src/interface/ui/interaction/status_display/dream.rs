//! 夢プール表示/ドリーム減少ポップアップの中継レイヤー（hw_ui 側実装へ委譲）

use bevy::prelude::*;

pub fn update_dream_loss_popup_ui_system(
    commands: Commands,
    time: Res<Time>,
    q_popups: Query<(
        Entity,
        &mut hw_ui::components::DreamLossPopupUi,
        &mut Node,
        &mut TextColor,
    )>,
) {
    hw_ui::interaction::status_display::update_dream_loss_popup_ui_system(commands, time, q_popups);
}

pub fn update_dream_pool_display_system(
    commands: Commands,
    time: Res<Time>,
    game_assets: Res<crate::assets::GameAssets>,
    dream_pool: Res<crate::entities::damned_soul::DreamPool>,
    theme: Res<hw_ui::theme::UiTheme>,
    ui_nodes: Res<hw_ui::components::UiNodeRegistry>,
    q_text: Query<(
        &mut Text,
        &mut TextColor,
        &mut hw_ui::components::DreamPoolPulse,
    )>,
) {
    hw_ui::interaction::status_display::update_dream_pool_display_system(
        commands,
        time,
        &game_assets.font_ui,
        dream_pool,
        theme,
        ui_nodes,
        q_text,
    );
}
