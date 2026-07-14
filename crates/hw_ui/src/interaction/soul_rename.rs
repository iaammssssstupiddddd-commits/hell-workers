use crate::components::{
    SoulRenameActive, SoulRenameButton, SoulRenameFieldContainer, SoulRenameState,
};
use crate::models::inspection::EntityInspectionViewModel;
use crate::setup::UiAssets;
use crate::theme::UiTheme;
use crate::widgets::{
    TextFieldConfig, TextFieldRole, focus_text_field, spawn_text_field_on_entity,
};
use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;

pub fn close_soul_rename(commands: &mut Commands, rename_state: &mut SoulRenameState) {
    if let Some(active) = rename_state.active.take() {
        commands.entity(active.field_root).despawn();
    }
}

#[derive(SystemParam)]
pub struct SoulRenameButtonCtx<'w, 's, A: UiAssets + Resource + 'static> {
    pub rename_state: ResMut<'w, SoulRenameState>,
    pub commands: Commands<'w, 's>,
    pub field_container: Query<'w, 's, Entity, With<SoulRenameFieldContainer>>,
    pub game_assets: Res<'w, A>,
    pub theme: Res<'w, UiTheme>,
    pub inspection_vm: Res<'w, EntityInspectionViewModel>,
    pub input_focus: ResMut<'w, InputFocus>,
}

/// Soul リネームボタンのクリック処理
pub fn soul_rename_button_system<A: UiAssets + Resource>(
    q_buttons: Query<&Interaction, (Changed<Interaction>, With<SoulRenameButton>)>,
    mut ctx: SoulRenameButtonCtx<A>,
) {
    for interaction in q_buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let Some(model) = ctx.inspection_vm.model.as_ref() else {
            continue;
        };
        if model.soul.is_none() {
            continue;
        }

        if ctx.rename_state.active.is_some() {
            close_soul_rename(&mut ctx.commands, &mut ctx.rename_state);
            continue;
        }

        let Ok(container) = ctx.field_container.single() else {
            continue;
        };

        let field = spawn_text_field_on_entity(
            &mut ctx.commands,
            container,
            ctx.game_assets.as_ref(),
            &ctx.theme,
            TextFieldConfig {
                initial_text: &model.header,
                role: TextFieldRole::SoulRename {
                    target: model.entity,
                },
                max_characters: Some(32),
                select_all_on_focus: true,
            },
        );

        ctx.rename_state.active = Some(SoulRenameActive {
            target: model.entity,
            field_root: field.root,
        });

        focus_text_field(&mut ctx.input_focus, field.editable);
    }
}

/// リネーム対象が変わった・パネルが閉じた場合に編集 UI を片付ける
pub fn soul_rename_cleanup_system(
    inspection_vm: Res<EntityInspectionViewModel>,
    mut rename_state: ResMut<SoulRenameState>,
    mut commands: Commands,
) {
    let Some(active) = rename_state.active else {
        return;
    };

    let should_close = inspection_vm
        .model
        .as_ref()
        .is_none_or(|model| model.soul.is_none() || model.entity != active.target);

    if should_close {
        close_soul_rename(&mut commands, &mut rename_state);
    }
}
