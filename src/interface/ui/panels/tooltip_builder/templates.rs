//! ツールチップテンプレート分岐

use crate::assets::GameAssets;
use crate::interface::ui::components::UiTooltip;
use crate::interface::ui::presentation::EntityInspectionModel;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

use super::widgets::{spawn_body_line, spawn_divider, spawn_header, spawn_icon_text_row, spawn_progress_bar};

fn parse_percent_value(text: &str) -> Option<f32> {
    let raw = text
        .split(':')
        .next_back()
        .unwrap_or(text)
        .trim()
        .trim_end_matches('%')
        .trim();
    raw.parse::<f32>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0))
}

pub fn build_soul_tooltip(
    parent: &mut ChildSpawnerCommands,
    model: Option<&EntityInspectionModel>,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let Some(model) = model else {
        build_generic_tooltip(parent, None, None, game_assets, theme);
        return;
    };

    let Some(soul) = &model.soul else {
        build_generic_tooltip(parent, Some(model), None, game_assets, theme);
        return;
    };

    spawn_header(
        parent,
        &format!("Soul: {}", model.header),
        game_assets,
        theme,
    );

    let motivation = parse_percent_value(&soul.motivation).unwrap_or(0.0);
    let stress = parse_percent_value(&soul.stress).unwrap_or(0.0);
    let fatigue = parse_percent_value(&soul.fatigue).unwrap_or(0.0);
    let stress_color = if stress >= 0.8 {
        theme.colors.status_danger
    } else if stress >= 0.5 {
        theme.colors.status_warning
    } else {
        theme.colors.status_healthy
    };

    spawn_progress_bar(
        parent,
        "Motivation",
        motivation,
        theme.colors.status_healthy,
        game_assets,
        theme,
    );
    spawn_progress_bar(parent, "Stress", stress, stress_color, game_assets, theme);
    spawn_progress_bar(
        parent,
        "Fatigue",
        fatigue,
        theme.colors.status_info,
        game_assets,
        theme,
    );

    spawn_divider(parent, theme);
    spawn_icon_text_row(parent, "TASK", &soul.task, game_assets, theme);
    spawn_icon_text_row(parent, "BAG", &soul.inventory, game_assets, theme);

    for line in soul
        .common
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        spawn_body_line(parent, line, game_assets, theme);
    }
}

pub fn build_building_tooltip(
    parent: &mut ChildSpawnerCommands,
    model: Option<&EntityInspectionModel>,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let Some(model) = model else {
        build_generic_tooltip(parent, None, None, game_assets, theme);
        return;
    };

    let title = if model.header.is_empty() {
        "Building".to_string()
    } else {
        model.header.clone()
    };
    spawn_header(parent, &title, game_assets, theme);

    let mut progress_added = false;
    for line in &model.tooltip_lines {
        if !progress_added
            && line.contains("Progress")
            && let Some(progress) = parse_percent_value(line)
        {
            spawn_progress_bar(
                parent,
                "Progress",
                progress,
                theme.colors.status_info,
                game_assets,
                theme,
            );
            progress_added = true;
            continue;
        }
        spawn_body_line(parent, line, game_assets, theme);
    }

    for line in model
        .common_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        spawn_body_line(parent, line, game_assets, theme);
    }
}

pub fn build_resource_tooltip(
    parent: &mut ChildSpawnerCommands,
    model: Option<&EntityInspectionModel>,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let Some(model) = model else {
        build_generic_tooltip(parent, None, None, game_assets, theme);
        return;
    };

    let title = if model.header.is_empty() {
        "Resource".to_string()
    } else {
        model.header.clone()
    };
    spawn_header(parent, &title, game_assets, theme);

    for line in model
        .tooltip_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
    {
        spawn_icon_text_row(parent, "RES", line, game_assets, theme);
    }

    for line in model
        .common_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        spawn_body_line(parent, line, game_assets, theme);
    }
}

pub fn build_ui_button_tooltip(
    parent: &mut ChildSpawnerCommands,
    tooltip: Option<&UiTooltip>,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let Some(tooltip) = tooltip else {
        spawn_header(parent, "UI Action", game_assets, theme);
        spawn_body_line(parent, "No tooltip text", game_assets, theme);
        return;
    };

    spawn_header(parent, tooltip.text, game_assets, theme);
    if let Some(shortcut) = tooltip.shortcut {
        spawn_divider(parent, theme);
        spawn_icon_text_row(
            parent,
            "KEY",
            &format!("Shortcut: {}", shortcut),
            game_assets,
            theme,
        );
    }
}

pub fn build_generic_tooltip(
    parent: &mut ChildSpawnerCommands,
    model: Option<&EntityInspectionModel>,
    ui_tooltip: Option<&UiTooltip>,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    if let Some(model) = model {
        let title = if model.header.is_empty() {
            "Entity".to_string()
        } else {
            model.header.clone()
        };
        spawn_header(parent, &title, game_assets, theme);

        for line in model
            .tooltip_lines
            .iter()
            .filter(|line| !line.trim().is_empty())
        {
            spawn_body_line(parent, line, game_assets, theme);
        }
        for line in model
            .common_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            spawn_body_line(parent, line, game_assets, theme);
        }
        return;
    }

    if let Some(tooltip) = ui_tooltip {
        spawn_header(parent, tooltip.text, game_assets, theme);
        if let Some(shortcut) = tooltip.shortcut {
            spawn_body_line(
                parent,
                &format!("Shortcut: {}", shortcut),
                game_assets,
                theme,
            );
        }
        return;
    }

    spawn_header(parent, "Tooltip", game_assets, theme);
}
