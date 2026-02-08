use crate::assets::GameAssets;
use crate::interface::ui::components::{
    TooltipBody, TooltipHeader, TooltipProgressBar, TooltipTemplate, UiTooltip,
};
use crate::interface::ui::list::clear_children;
use crate::interface::ui::presentation::EntityInspectionModel;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

const TOOLTIP_WRAP_LIMIT_BODY: usize = 42;
const TOOLTIP_WRAP_LIMIT_ICON_ROW: usize = 36;

pub fn rebuild_tooltip_content(
    commands: &mut Commands,
    tooltip_root: Entity,
    q_children: &Query<&Children>,
    game_assets: &GameAssets,
    theme: &UiTheme,
    template: TooltipTemplate,
    model: Option<&EntityInspectionModel>,
    ui_tooltip: Option<&UiTooltip>,
) {
    clear_children(commands, q_children, tooltip_root);

    commands
        .entity(tooltip_root)
        .with_children(|parent| match template {
            TooltipTemplate::Soul => build_soul_tooltip(parent, model, game_assets, theme),
            TooltipTemplate::Building => build_building_tooltip(parent, model, game_assets, theme),
            TooltipTemplate::Resource => build_resource_tooltip(parent, model, game_assets, theme),
            TooltipTemplate::UiButton => {
                build_ui_button_tooltip(parent, ui_tooltip, game_assets, theme)
            }
            TooltipTemplate::Generic => {
                build_generic_tooltip(parent, model, ui_tooltip, game_assets, theme)
            }
        });
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

pub fn spawn_progress_bar(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: f32,
    color: Color,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let clamped = value.clamp(0.0, 1.0);
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .with_children(|bar_col| {
            bar_col.spawn((
                Text::new(format!("{label}: {:.0}%", clamped * 100.0)),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_xs,
                    ..default()
                },
                TextColor(theme.colors.text_secondary_semantic),
                TooltipBody,
            ));

            bar_col
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        margin: UiRect::top(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                ))
                .with_children(|track| {
                    track.spawn((
                        Node {
                            width: Val::Percent(clamped * 100.0),
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(color),
                        TooltipProgressBar(clamped),
                    ));
                });
        });
}

pub fn spawn_divider(parent: &mut ChildSpawnerCommands, theme: &UiTheme) {
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(theme.colors.border_default),
    ));
}

pub fn spawn_icon_text_row(
    parent: &mut ChildSpawnerCommands,
    icon: &str,
    text: &str,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Start,
            column_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(icon),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_xs,
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_accent_semantic),
                TooltipBody,
            ));
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                ..default()
            })
            .with_children(|text_col| {
                for line in wrap_tooltip_text(text, TOOLTIP_WRAP_LIMIT_ICON_ROW) {
                    text_col.spawn((
                        Text::new(line),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_sm,
                            ..default()
                        },
                        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                        TextColor(theme.colors.text_primary_semantic),
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        TooltipBody,
                    ));
                }
            });
        });
}

fn spawn_header(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let display_text = wrap_tooltip_text(text, TOOLTIP_WRAP_LIMIT_BODY).join("\n");
    parent.spawn((
        Text::new(display_text),
        TextFont {
            font: game_assets.font_ui.clone(),
            font_size: theme.typography.font_size_md,
            weight: FontWeight::BOLD,
            ..default()
        },
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        TextColor(theme.colors.text_accent_semantic),
        Node {
            width: Val::Percent(100.0),
            ..default()
        },
        TooltipHeader,
    ));
}

fn spawn_body_line(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    for line in wrap_tooltip_text(text, TOOLTIP_WRAP_LIMIT_BODY) {
        parent.spawn((
            Text::new(line),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_sm,
                ..default()
            },
            TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
            TextColor(theme.colors.text_primary_semantic),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            TooltipBody,
        ));
    }
}

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

fn wrap_tooltip_text(text: &str, limit: usize) -> Vec<String> {
    let mut output = Vec::new();
    for raw_line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut remaining = raw_line.to_string();
        while remaining.chars().count() > limit {
            let split_idx = preferred_split_index(&remaining, limit)
                .or_else(|| whitespace_split_index(&remaining, limit))
                .unwrap_or_else(|| char_to_byte_idx(&remaining, limit));
            let (head, tail) = remaining.split_at(split_idx);
            output.push(head.trim().to_string());
            remaining = tail
                .trim_start_matches(|ch: char| ch.is_whitespace() || [',', ';', '|'].contains(&ch))
                .to_string();
            if remaining.is_empty() {
                break;
            }
        }
        if !remaining.is_empty() {
            output.push(remaining);
        }
    }

    if output.is_empty() {
        output.push(String::new());
    }
    output
}

fn preferred_split_index(text: &str, limit: usize) -> Option<usize> {
    let limit_byte = char_to_byte_idx(text, limit);
    let mut best: Option<usize> = None;
    for pattern in [": ", ", ", " - ", " | ", "; "] {
        if let Some((idx, _)) = text[..limit_byte].rmatch_indices(pattern).next() {
            let split_at = if pattern == ": " { idx + 1 } else { idx };
            if split_at > 0 {
                best = best.max(Some(split_at));
            }
        }
    }
    best
}

fn whitespace_split_index(text: &str, limit: usize) -> Option<usize> {
    let limit_byte = char_to_byte_idx(text, limit);
    text[..limit_byte]
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| ch.is_whitespace().then_some(idx))
}

fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

