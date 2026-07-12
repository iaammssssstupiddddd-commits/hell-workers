use crate::components::UiInputState;
use crate::list::dirty::EntityListDirty;
use crate::list::search::EntityListSearchState;
use crate::text_input_intents::TextInputIntent;
use crate::widgets::{TextFieldRole, TextFieldRoot, editable_text_value};
use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusGained, FocusLost, FocusedInput, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;

/// Enter / Escape 確定を observer から system へ渡すバッファ
#[derive(Resource, Default, Debug)]
pub struct TextFieldPendingAction {
    pub pending: Option<TextFieldAction>,
}

#[derive(Debug, Clone)]
pub enum TextFieldAction {
    SubmitRename { entity: Entity, name: String },
    CancelSearch { editable: Entity },
    CancelRename,
    ClearFocus,
}

/// フレーム開始時に consumed latch をリセットする
pub fn reset_text_input_consumed_keyboard_system(mut ui_input_state: ResMut<UiInputState>) {
    ui_input_state.text_input_consumed_keyboard = false;
}

/// `InputFocus` から `UiInputState.text_input_focused` を同期する
pub fn text_input_focus_sync_system(
    input_focus: Res<InputFocus>,
    q_editable: Query<(), With<EditableText>>,
    mut ui_input_state: ResMut<UiInputState>,
) {
    ui_input_state.text_input_focused = input_focus
        .get()
        .is_some_and(|entity| q_editable.get(entity).is_ok());
}

pub fn is_editable_text_focused(
    input_focus: &InputFocus,
    q_editable: &Query<(), With<EditableText>>,
) -> bool {
    input_focus
        .get()
        .is_some_and(|entity| q_editable.get(entity).is_ok())
}

/// テキスト入力中はゲーム keybind を抑止する
pub fn text_input_blocks_keybinds(ui_input_state: &UiInputState) -> bool {
    ui_input_state.text_input_blocks_keybinds()
}

/// フォーカス時に枠線色を更新する
pub fn on_text_field_focus_gained(
    focus_gained: On<FocusGained>,
    q_editable: Query<&crate::widgets::TextFieldEditable>,
    theme: Res<crate::theme::UiTheme>,
    mut q_roots: Query<&mut BorderColor, With<TextFieldRoot>>,
) {
    let target = focus_gained.event_target();
    let Ok(editable) = q_editable.get(target) else {
        return;
    };
    if let Ok(mut border) = q_roots.get_mut(editable.root) {
        *border = BorderColor::all(theme.colors.panel_accent_info_panel);
    }
}

/// フォーカス喪失時に枠線色を戻す
pub fn on_text_field_focus_lost(
    focus_lost: On<FocusLost>,
    q_editable: Query<&crate::widgets::TextFieldEditable>,
    theme: Res<crate::theme::UiTheme>,
    mut q_roots: Query<&mut BorderColor, With<TextFieldRoot>>,
) {
    let target = focus_lost.event_target();
    let Ok(editable) = q_editable.get(target) else {
        return;
    };
    if let Ok(mut border) = q_roots.get_mut(editable.root) {
        *border = BorderColor::all(theme.colors.border_default);
    }
}

/// Enter / Escape を検出して pending action に積む
pub fn on_text_field_keyboard_input(
    input: On<FocusedInput<KeyboardInput>>,
    q_fields: Query<(&EditableText, &TextFieldRole)>,
    mut pending: ResMut<TextFieldPendingAction>,
    mut ui_input_state: ResMut<UiInputState>,
) {
    if !input.input.state.is_pressed() {
        return;
    }

    let Ok((editable, role)) = q_fields.get(input.focused_entity) else {
        return;
    };

    if editable.is_composing() {
        return;
    }

    ui_input_state.text_input_consumed_keyboard = true;

    pending.pending = Some(match (&input.input.logical_key, role) {
        (Key::Enter, TextFieldRole::SoulRename { target }) => TextFieldAction::SubmitRename {
            entity: *target,
            name: editable_text_value(editable),
        },
        (Key::Enter, TextFieldRole::DevPoc | TextFieldRole::EntityListSearch) => {
            TextFieldAction::ClearFocus
        }
        (Key::Escape, TextFieldRole::EntityListSearch) => TextFieldAction::CancelSearch {
            editable: input.focused_entity,
        },
        (Key::Escape, TextFieldRole::SoulRename { .. }) => TextFieldAction::CancelRename,
        (Key::Escape, TextFieldRole::DevPoc) => TextFieldAction::ClearFocus,
        _ => return,
    });
}

#[derive(SystemParam)]
pub struct TextFieldActionCtx<'w, 's> {
    pub input_focus: ResMut<'w, InputFocus>,
    pub text_intents: MessageWriter<'w, TextInputIntent>,
    pub search_state: ResMut<'w, EntityListSearchState>,
    pub entity_list_dirty: ResMut<'w, EntityListDirty>,
    pub rename_state: ResMut<'w, crate::components::SoulRenameState>,
    pub q_editable: Query<'w, 's, &'static mut EditableText>,
    pub commands: Commands<'w, 's>,
}

/// pending action を適用する
pub fn apply_text_field_pending_action_system(
    mut pending: ResMut<TextFieldPendingAction>,
    mut ctx: TextFieldActionCtx,
) {
    let Some(action) = pending.pending.take() else {
        return;
    };

    match action {
        TextFieldAction::SubmitRename { entity, name } => {
            ctx.text_intents
                .write(TextInputIntent::RenameSoul { entity, name });
            crate::interaction::soul_rename::close_soul_rename(
                &mut ctx.commands,
                &mut ctx.rename_state,
            );
            ctx.input_focus.clear();
        }
        TextFieldAction::CancelSearch { editable } => {
            if let Ok(mut editable) = ctx.q_editable.get_mut(editable) {
                editable.clear();
            }
            if !ctx.search_state.query.is_empty() {
                ctx.search_state.query.clear();
                ctx.search_state.last_applied.clear();
                ctx.entity_list_dirty.mark_structure();
            }
            ctx.input_focus.clear();
        }
        TextFieldAction::CancelRename => {
            crate::interaction::soul_rename::close_soul_rename(
                &mut ctx.commands,
                &mut ctx.rename_state,
            );
            ctx.input_focus.clear();
        }
        TextFieldAction::ClearFocus => {
            ctx.input_focus.clear();
        }
    }
}

/// `EntityListSearch` のライブ更新
pub fn sync_entity_list_search_system(
    q_search_fields: Query<
        (&EditableText, &TextFieldRole),
        With<crate::widgets::TextFieldEditable>,
    >,
    mut search_state: ResMut<EntityListSearchState>,
    mut entity_list_dirty: ResMut<EntityListDirty>,
) {
    for (editable, role) in q_search_fields.iter() {
        if !matches!(role, TextFieldRole::EntityListSearch) {
            continue;
        }
        let value = editable_text_value(editable);
        if value == search_state.query {
            continue;
        }
        search_state.query = value;
        if search_state.normalized() != search_state.last_applied {
            entity_list_dirty.mark_structure();
        }
    }
}

/// structure sync 後に last_applied を更新する
pub fn finalize_entity_list_search_apply_system(
    dirty: Res<EntityListDirty>,
    mut search_state: ResMut<EntityListSearchState>,
) {
    if dirty.needs_structure_sync() {
        search_state.last_applied = search_state.query.trim().to_string();
    }
}
