use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle};
use bevy::ui_widgets::SelectAllOnFocus;

/// テキストフィールドの用途識別（observer フィルタ用）
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldRole {
    /// エンティティリスト検索（ライブフィルタ、Enter で確定不要）
    EntityListSearch,
    /// Soul リネーム（Enter=確定、Escape=キャンセル）
    SoulRename { target: Entity },
    /// M1 PoC 用
    DevPoc,
}

/// テキストフィールド外枠/root
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextFieldRoot;

/// EditableText から枠線/root を更新するための逆参照
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextFieldEditable {
    pub root: Entity,
}

/// スポーン済みテキストフィールドのハンドル
pub struct TextFieldHandle {
    pub root: Entity,
    pub editable: Entity,
}

pub struct TextFieldConfig<'a> {
    pub initial_text: &'a str,
    pub role: TextFieldRole,
    pub max_characters: Option<usize>,
    pub select_all_on_focus: bool,
}

impl<'a> TextFieldConfig<'a> {
    pub const fn new(initial_text: &'a str, role: TextFieldRole) -> Self {
        Self {
            initial_text,
            role,
            max_characters: None,
            select_all_on_focus: false,
        }
    }
}

pub fn editable_text_value(editable: &EditableText) -> String {
    editable
        .value()
        .into_iter()
        .fold(String::new(), |mut acc, segment| {
            acc.push_str(segment);
            acc
        })
}

pub fn spawn_text_field(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    config: TextFieldConfig<'_>,
) -> TextFieldHandle {
    let mut editable = EditableText::new(config.initial_text);
    editable.allow_newlines = false;
    editable.max_characters = config.max_characters;

    let mut editable_entity = Entity::PLACEHOLDER;
    let root = parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(24.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(theme.colors.bg_surface),
            BorderColor::all(theme.colors.border_default),
            TextFieldRoot,
        ))
        .with_children(|child| {
            editable_entity = child
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(20.0),
                        ..default()
                    },
                    editable,
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: crate::theme::font_size_rem(theme.typography.font_size_small),
                        ..default()
                    },
                    TextColor(theme.colors.text_primary_semantic),
                    TextCursorStyle {
                        color: theme.colors.text_primary_semantic,
                        selected_text_color: Some(theme.colors.bg_surface),
                        ..default()
                    },
                    config.role,
                ))
                .id();
        })
        .id();

    parent
        .commands()
        .entity(editable_entity)
        .insert(TextFieldEditable { root });
    if config.select_all_on_focus {
        parent
            .commands()
            .entity(editable_entity)
            .insert(SelectAllOnFocus);
    }

    TextFieldHandle {
        root,
        editable: editable_entity,
    }
}

/// フォーカスをテキストフィールドへ移す
pub fn focus_text_field(input_focus: &mut InputFocus, editable: Entity) {
    input_focus.set(editable, FocusCause::Navigated);
}

pub fn spawn_text_field_on_entity(
    commands: &mut Commands,
    parent: Entity,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    config: TextFieldConfig<'_>,
) -> TextFieldHandle {
    let mut handle = TextFieldHandle {
        root: Entity::PLACEHOLDER,
        editable: Entity::PLACEHOLDER,
    };
    commands.entity(parent).with_children(|child| {
        handle = spawn_text_field(child, game_assets, theme, config);
    });
    handle
}
