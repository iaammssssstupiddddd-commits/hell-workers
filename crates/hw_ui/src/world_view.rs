//! Explicit map layers. A tool can temporarily override the chosen layer.
use bevy::prelude::*;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldView {
    #[default]
    None,
    Duties,
    Storage,
    Power,
    Rooms,
}

impl WorldView {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Duties,
        Self::Storage,
        Self::Power,
        Self::Rooms,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "通常表示",
            Self::Duties => "担当範囲",
            Self::Storage => "保管場所",
            Self::Power => "給電状態",
            Self::Rooms => "成立した部屋",
        }
    }
    pub const fn legend(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Duties => "担当範囲 · 四角は作業範囲 / 円は指揮が届く範囲",
            Self::Storage => "保管場所 · 青い四角が保管タイル / 線は選択した魂の実際の作業先",
            Self::Power => "給電状態 · 白い丸は給電中 / 橙の×は未給電",
            Self::Rooms => "成立した部屋 · 枠は完成した床と壁で囲まれた室内",
        }
    }
}

#[derive(Resource, Default)]
pub struct WorldViewState {
    pub manual: WorldView,
    pub temporary: Option<WorldView>,
}

impl WorldViewState {
    pub fn effective(&self) -> WorldView {
        self.temporary.unwrap_or(self.manual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tool_override_restores_the_manual_layer() {
        let mut view = WorldViewState {
            manual: WorldView::Rooms,
            temporary: Some(WorldView::Storage),
        };
        assert_eq!(view.effective(), WorldView::Storage);
        view.temporary = None;
        assert_eq!(view.effective(), WorldView::Rooms);
    }
}

#[derive(Component)]
pub struct WorldViewPanel;
#[derive(Component)]
pub struct WorldViewLegend;

pub fn spawn_world_view_ui(
    commands: &mut Commands,
    parent: Entity,
    assets: &dyn crate::setup::UiAssets,
    theme: &crate::theme::UiTheme,
) {
    let font = TextFont {
        font: assets.font_ui().clone().into(),
        font_size: crate::theme::font_size_rem(14.0),
        ..default()
    };
    let panel = commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(64.0),
                width: Val::Px(296.0),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            WorldViewPanel,
            BackgroundColor(theme.colors.bg_surface),
            crate::components::UiInputBlocker,
            bevy::ui::RelativeCursorPosition::default(),
        ))
        .with_children(|panel| {
            crate::shell::spawn_workspace_navigation(panel, assets, theme, false);
            panel.spawn((
                Text::new("地図で確認する情報"),
                font.clone(),
                TextColor(theme.colors.text_primary_semantic),
            ));
            for view in WorldView::ALL {
                panel
                    .spawn((
                        Button,
                        Node {
                            min_height: Val::Px(36.0),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(theme.colors.button_default),
                        crate::components::MenuButton(crate::UiIntent::SetWorldView(view)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(view.label()),
                            font.clone(),
                            TextColor(theme.colors.text_primary_semantic),
                        ));
                    });
            }
        })
        .id();
    let legend = commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(64.0),
                max_width: Val::Vw(62.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            WorldViewLegend,
            Text::new(""),
            font,
            TextColor(theme.colors.text_primary_semantic),
            BackgroundColor(theme.colors.bg_surface),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_children(&[panel, legend]);
}

type LegendNodes<'w, 's> = Query<
    'w,
    's,
    (&'static mut Node, &'static mut Text),
    (With<WorldViewLegend>, Without<WorldViewPanel>),
>;

pub fn world_view_ui_system(
    shell: Res<crate::shell::UiShellState>,
    view: Res<WorldViewState>,
    mut panels: Query<&mut Node, (With<WorldViewPanel>, Without<WorldViewLegend>)>,
    mut legends: LegendNodes,
) {
    for mut node in &mut panels {
        let display = if shell.page == crate::shell::WorkspacePage::Display {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (mut node, mut text) in &mut legends {
        let display = if view.effective() == WorldView::None {
            Display::None
        } else {
            Display::Flex
        };
        if node.display != display {
            node.display = display;
        }
        let legend = view.effective().legend();
        if text.0 != legend {
            text.0 = legend.to_owned();
        }
    }
}
