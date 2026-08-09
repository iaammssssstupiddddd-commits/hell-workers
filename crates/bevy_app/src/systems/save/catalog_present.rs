//! Presents SaveCatalogUi state onto the SaveCatalogDialog tree.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::{
    MenuAction, MenuButton, SaveCatalogDialog, SaveCatalogSlotList, SaveCatalogTitle,
};
use hw_ui::setup::UiAssets;
use hw_ui::theme::UiTheme;

use crate::assets::GameAssets;
use crate::systems::save::catalog::{SaveCatalogEntry, format_relative_modified};
use crate::systems::save::{SaveCatalog, SaveCatalogMode, SaveCatalogUi};

#[derive(SystemParam)]
pub(crate) struct CatalogPresentationParams<'w, 's> {
    theme: Res<'w, UiTheme>,
    assets: Res<'w, GameAssets>,
    q_dialog: Query<'w, 's, &'static mut Node, With<SaveCatalogDialog>>,
    q_title: Query<'w, 's, &'static mut Text, With<SaveCatalogTitle>>,
    q_lists: Query<'w, 's, Entity, With<SaveCatalogSlotList>>,
    commands: Commands<'w, 's>,
    children: Query<'w, 's, &'static Children>,
}

fn catalog_entry_action(
    mode: SaveCatalogMode,
    entry: &SaveCatalogEntry,
    session: u64,
) -> Option<MenuAction> {
    let slot = entry.slot;
    match mode {
        SaveCatalogMode::SaveCatalog if entry.capabilities.can_manual_save => {
            Some(MenuAction::SelectSaveCatalogSlot { slot, session })
        }
        SaveCatalogMode::OverwriteConfirm { slot: confirmed } if confirmed == slot => {
            Some(MenuAction::ConfirmSaveCatalogSlot { slot, session })
        }
        SaveCatalogMode::LoadCatalog | SaveCatalogMode::RecoveryLoadCatalog
            if entry.capabilities.can_load =>
        {
            Some(MenuAction::SelectLoadCatalogSlot { slot, session })
        }
        SaveCatalogMode::LoadConfirm {
            slot: confirmed, ..
        } if confirmed == slot && entry.capabilities.can_load => {
            Some(MenuAction::ConfirmLoadCatalogSlot { slot, session })
        }
        _ => None,
    }
}

pub(crate) fn sync_save_catalog_dialog_system(
    catalog_ui: Res<SaveCatalogUi>,
    catalog: Res<SaveCatalog>,
    mut params: CatalogPresentationParams,
) {
    let visible = catalog_ui.is_open();
    for mut node in &mut params.q_dialog {
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    if !visible {
        return;
    }
    if !(catalog_ui.is_changed() || catalog.is_changed()) {
        return;
    }

    let title = match catalog_ui.mode {
        SaveCatalogMode::SaveCatalog | SaveCatalogMode::OverwriteConfirm { .. } => "Save Game",
        SaveCatalogMode::LoadCatalog
        | SaveCatalogMode::LoadConfirm {
            recovery: false, ..
        } => "Load Game",
        SaveCatalogMode::RecoveryLoadCatalog
        | SaveCatalogMode::LoadConfirm { recovery: true, .. } => "Recovery Load",
        SaveCatalogMode::Closed => return,
    };
    for mut text in &mut params.q_title {
        *text = Text::new(title);
    }

    let Ok(list_entity) = params.q_lists.single() else {
        return;
    };
    if let Ok(existing) = params.children.get(list_entity) {
        for child in existing.iter() {
            params.commands.entity(child).despawn();
        }
    }

    let session = catalog_ui.session;
    let now = std::time::SystemTime::now();
    for entry in catalog.entries() {
        let label = format!(
            "{} — {} — {}",
            entry.player_label(),
            entry.content_label(),
            if entry.modified_unavailable {
                "Modified time unavailable".to_owned()
            } else {
                format_relative_modified(entry.modified, now)
            }
        );
        let action = catalog_entry_action(catalog_ui.mode, entry, session);
        let row = params
            .commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(36.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if action.is_some() {
                    params.theme.colors.button_default
                } else {
                    params.theme.colors.dialog_bg
                }),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new(label),
                    TextFont {
                        font: params.assets.font_ui().clone().into(),
                        font_size: FontSize::Px(params.theme.typography.font_size_dialog_small),
                        ..default()
                    },
                    TextColor(if action.is_some() {
                        Color::WHITE
                    } else {
                        params.theme.colors.text_muted
                    }),
                ));
            })
            .id();
        if let Some(action) = action {
            params
                .commands
                .entity(row)
                .insert((Button, MenuButton(action)));
        }
        params.commands.entity(list_entity).add_child(row);
    }

    if matches!(
        catalog_ui.mode,
        SaveCatalogMode::OverwriteConfirm { .. } | SaveCatalogMode::LoadConfirm { .. }
    ) {
        let cancel = params
            .commands
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(32.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(params.theme.colors.button_default),
                MenuButton(MenuAction::CancelSaveCatalogConfirm),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Back"),
                    TextFont {
                        font: params.assets.font_ui().clone().into(),
                        font_size: FontSize::Px(params.theme.typography.font_size_dialog_small),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            })
            .id();
        params.commands.entity(list_entity).add_child(cancel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::{SaveContentStatus, SaveFileRevision, SaveSlotCapabilities};
    use hw_core::{SaveSlotId, SaveSlotRole};

    fn entry(content: SaveContentStatus, can_load: bool) -> SaveCatalogEntry {
        SaveCatalogEntry {
            slot: SaveSlotId::Manual1,
            role: SaveSlotRole::Manual,
            capabilities: SaveSlotCapabilities {
                can_manual_save: true,
                can_scheduler_save: false,
                can_load,
            },
            content,
            revision: SaveFileRevision::absent(),
            file_size: 0,
            modified: None,
            modified_unavailable: false,
        }
    }

    #[test]
    fn load_catalog_keeps_non_loadable_rows_visible_but_without_actions() {
        for content in [
            SaveContentStatus::Empty,
            SaveContentStatus::CorruptHeader,
            SaveContentStatus::UnsupportedVersion { found: 2 },
            SaveContentStatus::SeedMismatch { worldgen_seed: 9 },
            SaveContentStatus::Unreadable,
        ] {
            assert!(
                catalog_entry_action(SaveCatalogMode::LoadCatalog, &entry(content, false), 4)
                    .is_none(),
                "{content:?} must remain a disabled row"
            );
        }

        for content in [
            SaveContentStatus::LegacyV0Candidate,
            SaveContentStatus::BodyInvalid,
        ] {
            assert!(matches!(
                catalog_entry_action(SaveCatalogMode::LoadCatalog, &entry(content, true), 4),
                Some(MenuAction::SelectLoadCatalogSlot { .. })
            ));
        }
    }
}
