//! Presents SaveCatalogUi state onto the SaveCatalogDialog tree.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::{
    MenuAction, MenuButton, SaveCatalogConfirmFooter, SaveCatalogDialog, SaveCatalogSlotList,
    SaveCatalogTitle,
};
use hw_ui::setup::UiAssets;
use hw_ui::theme::UiTheme;

use crate::assets::GameAssets;
use crate::systems::save::catalog::{SaveCatalogEntry, format_relative_modified};
use crate::systems::save::{SaveCatalog, SaveCatalogMode, SaveCatalogUi};

/// Cached filesystem timestamp, explicitly UTC; never guesses the host timezone.
fn absolute_modified(modified: Option<std::time::SystemTime>) -> Option<String> {
    let seconds = modified?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut days = seconds / 86_400;
    let mut year = 1970;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let year_days = if leap { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
        if year > 9999 {
            return None;
        }
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for length in months {
        if days < length {
            break;
        }
        days -= length;
        month += 1;
    }
    Some(format!(
        "{year:04}-{month:02}-{:02} {:02}:{:02}:{:02} UTC",
        days + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    ))
}

fn catalog_entry_label(entry: &SaveCatalogEntry, now: std::time::SystemTime) -> String {
    let role = match entry.role {
        hw_core::SaveSlotRole::Manual => "手動保存",
        hw_core::SaveSlotRole::Autosave => "自動保存",
        hw_core::SaveSlotRole::LegacyDefault => "旧形式スロット",
    };
    let modified = if !entry.revision.exists {
        "保存日時: —".to_owned()
    } else if entry.modified_unavailable {
        "保存日時を取得できません".to_owned()
    } else if let Some(absolute) = absolute_modified(entry.modified) {
        format!(
            "{absolute}（{}）",
            format_relative_modified(entry.modified, now)
        )
    } else {
        "保存日時を表示できません".to_owned()
    };
    format!(
        "{role} / {}\n{}\n{modified}",
        entry.player_label(),
        entry.content_label()
    )
}

#[derive(SystemParam)]
pub(crate) struct CatalogPresentationParams<'w, 's> {
    theme: Res<'w, UiTheme>,
    assets: Res<'w, GameAssets>,
    q_dialog: Query<'w, 's, &'static mut Node, With<SaveCatalogDialog>>,
    q_title: Query<'w, 's, &'static mut Text, With<SaveCatalogTitle>>,
    q_lists: Query<'w, 's, Entity, With<SaveCatalogSlotList>>,
    q_footers: Query<'w, 's, Entity, With<SaveCatalogConfirmFooter>>,
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
        SaveCatalogMode::LoadCatalog | SaveCatalogMode::RecoveryLoadCatalog
            if entry.capabilities.can_load =>
        {
            Some(MenuAction::SelectLoadCatalogSlot { slot, session })
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
    let Ok(footer) = params.q_footers.single() else {
        return;
    };
    if let Ok(existing) = params.children.get(footer) {
        for child in existing.iter() {
            params.commands.entity(child).despawn();
        }
    }
    if let Ok(existing) = params.children.get(list_entity) {
        for child in existing.iter() {
            params.commands.entity(child).despawn();
        }
    }

    let session = catalog_ui.session;
    let confirmation = match catalog_ui.mode {
        SaveCatalogMode::OverwriteConfirm { slot } => Some((
            slot,
            "上書きすると、このスロットの保存内容は置き換わります。",
            MenuAction::ConfirmSaveCatalogSlot { slot, session },
        )),
        SaveCatalogMode::LoadConfirm { slot, .. } => Some((
            slot,
            "ロードすると現在のワールドは置き換わります。未保存の変更は失われます。",
            MenuAction::ConfirmLoadCatalogSlot { slot, session },
        )),
        _ => None,
    };
    if let Some((slot, explanation, action)) = confirmation {
        let label = catalog.entry(slot).map_or_else(
            || "利用できないスロット".to_owned(),
            |entry| catalog_entry_label(entry, std::time::SystemTime::now()),
        );
        params.commands.entity(list_entity).with_children(|parent| {
            parent.spawn((
                Text::new(format!("{label}\n\n{explanation}")),
                TextFont {
                    font: params.assets.font_ui().clone().into(),
                    font_size: FontSize::Px(params.theme.typography.font_size_dialog_small),
                    ..default()
                },
                TextColor(params.theme.colors.text_primary_semantic),
            ));
        });
        for (label, action) in [
            ("Back", MenuAction::CancelSaveCatalogConfirm),
            ("Confirm", action),
        ] {
            params.commands.entity(footer).with_children(|parent| {
                parent
                    .spawn((
                        Button,
                        MenuButton(action),
                        Node {
                            min_width: Val::Px(100.0),
                            height: Val::Px(36.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(params.theme.colors.button_default),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font: params.assets.font_ui().clone().into(),
                                font_size: FontSize::Px(
                                    params.theme.typography.font_size_dialog_small,
                                ),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });
        }
        return;
    }
    let now = std::time::SystemTime::now();
    for entry in catalog.entries() {
        let label = catalog_entry_label(entry, now);
        let action = catalog_entry_action(catalog_ui.mode, entry, session);
        let row = params
            .commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(36.0),
                    flex_shrink: 0.0,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::{SaveContentStatus, SaveFileRevision, SaveSlotCapabilities};
    use hw_core::{SaveSlotId, SaveSlotRole};

    #[test]
    fn save_dates_show_utc_and_handle_calendar_boundaries() {
        use std::time::{Duration, UNIX_EPOCH};
        for (seconds, expected) in [
            (0, "1970-01-01 00:00:00 UTC"),
            (951_782_400, "2000-02-29 00:00:00 UTC"),
            (4_107_542_400, "2100-03-01 00:00:00 UTC"),
            (1_735_689_599, "2024-12-31 23:59:59 UTC"),
        ] {
            assert_eq!(
                absolute_modified(Some(UNIX_EPOCH + Duration::from_secs(seconds))).as_deref(),
                Some(expected)
            );
        }
        assert_eq!(absolute_modified(None), None);
        assert_eq!(
            absolute_modified(UNIX_EPOCH.checked_sub(Duration::from_secs(1))),
            None
        );
        assert_eq!(
            absolute_modified(UNIX_EPOCH.checked_add(Duration::from_secs(253_402_300_800))),
            None
        );
    }

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
