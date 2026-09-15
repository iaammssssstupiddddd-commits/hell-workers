use super::{AreaEditClipboard, AreaEditHistory, AreaEditPresets, AreaEditSession};
use crate::app_contexts::TaskContext;
use crate::input_actions::{InputAction, PendingWorldInputCapture};
use crate::interface::selection::SelectedEntity;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::familiar::Familiar;
use hw_core::{WorldEpoch, constants::TILE_SIZE};
use hw_ui::area_edit::{
    AreaEditHistoryEntry,
    panel::{AreaEditAction, AreaEditControl, AreaEditPanelModel},
};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct AreaUiFlags {
    pub epoch: u64,
    pub visible: bool,
    pub enabled: bool,
}

#[derive(Clone, PartialEq)]
pub(super) struct AreaUiSnapshot {
    selected: Option<Entity>,
    name: Option<String>,
    area: Option<TaskArea>,
    undo: Option<(AreaEditHistoryEntry, Option<String>)>,
    redo: Option<(AreaEditHistoryEntry, Option<String>)>,
    counts: (usize, usize),
    clipboard: Option<TaskArea>,
    presets: [Option<Vec2>; 3],
    flags: AreaUiFlags,
}

#[derive(Resource, Default)]
pub struct AreaEditUiState {
    pub(super) revision: u64,
    pub(super) snapshot: Option<AreaUiSnapshot>,
}

#[derive(SystemParam)]
pub struct AreaUiGuard<'w> {
    epoch: Res<'w, WorldEpoch>,
    task: Res<'w, TaskContext>,
    session: Res<'w, AreaEditSession>,
    recovery: Option<Res<'w, crate::systems::save::SaveRecoveryMode>>,
    input: Res<'w, hw_ui::components::UiInputState>,
    pending: Res<'w, PendingWorldInputCapture>,
}

impl AreaUiGuard<'_> {
    pub(super) fn flags(&self) -> AreaUiFlags {
        AreaUiFlags {
            epoch: self.epoch.get(),
            visible: matches!(self.task.0, TaskMode::AreaSelection(_)),
            enabled: matches!(self.task.0, TaskMode::AreaSelection(None))
                && !self.session.is_dragging()
                && !self.recovery.as_ref().is_some_and(|recovery| {
                    **recovery == crate::systems::save::SaveRecoveryMode::RecoveryFailed
                })
                && !self.input.world_pointer_claimed
                && !self.input.world_input_captured
                && !self.input.text_input_blocks_keybinds()
                && self.pending.overlay().is_none(),
        }
    }
}

pub(super) fn snapshot(
    selected: Option<Entity>,
    history: &AreaEditHistory,
    clipboard: &AreaEditClipboard,
    presets: &AreaEditPresets,
    queries: (&Query<&Familiar>, &Query<&TaskArea, With<Familiar>>),
    flags: AreaUiFlags,
) -> AreaUiSnapshot {
    let (familiars, areas) = queries;
    let name = |entity| {
        familiars
            .get(entity)
            .ok()
            .map(|familiar| familiar.name.clone())
    };
    let entry = |entry: &AreaEditHistoryEntry| (entry.clone(), name(entry.familiar_entity));
    AreaUiSnapshot {
        selected,
        name: selected.and_then(name),
        area: selected.and_then(|entity| areas.get(entity).ok()).cloned(),
        undo: history.undo_stack.last().map(entry),
        redo: history.redo_stack.last().map(entry),
        counts: (history.undo_stack.len(), history.redo_stack.len()),
        clipboard: clipboard.area.clone(),
        presets: presets.slots,
        flags,
    }
}

fn dimensions(size: Vec2) -> String {
    format!(
        "{:.0}×{:.0} タイル",
        size.x.abs() / TILE_SIZE,
        size.y.abs() / TILE_SIZE
    )
}

impl AreaUiSnapshot {
    fn controls(&self) -> Vec<AreaEditControl> {
        let mut controls = Vec::new();
        let mut add = |action, label, enabled| {
            controls.push(AreaEditControl {
                action,
                label,
                enabled: enabled && self.flags.enabled,
            })
        };
        for (action, verb, history, use_before) in [
            (AreaEditAction::Undo, "戻す", &self.undo, true),
            (AreaEditAction::Redo, "やり直す", &self.redo, false),
        ] {
            let (label, enabled) = match history {
                Some((entry, Some(name))) => {
                    let area = if use_before {
                        &entry.before
                    } else {
                        &entry.after
                    };
                    let size = area
                        .as_ref()
                        .map(|area| dimensions(area.size()))
                        .unwrap_or_else(|| "範囲なし".into());
                    (format!("{verb}: {name} → {size}"), true)
                }
                Some(_) => (format!("{verb}: 担当の使い魔がいません"), false),
                None => (format!("{verb}: 履歴がありません"), false),
            };
            add(action, label, enabled);
        }
        let name = self.name.as_deref().unwrap_or("使い魔未選択");
        let current = self.area.as_ref().map(|area| dimensions(area.size()));
        add(
            AreaEditAction::Copy,
            format!(
                "コピー: {name} / {}",
                current.as_deref().unwrap_or("範囲なし")
            ),
            self.name.is_some() && self.area.is_some(),
        );
        add(
            AreaEditAction::Paste,
            format!(
                "貼り付け先: {name} / {}",
                self.clipboard
                    .as_ref()
                    .map(|area| dimensions(area.size()))
                    .unwrap_or_else(|| "コピーがありません".into())
            ),
            self.name.is_some() && self.clipboard.is_some(),
        );
        for (slot, (save, load)) in [
            (AreaEditAction::Save1, AreaEditAction::Load1),
            (AreaEditAction::Save2, AreaEditAction::Load2),
            (AreaEditAction::Save3, AreaEditAction::Load3),
        ]
        .into_iter()
        .enumerate()
        {
            add(
                save,
                format!(
                    "枠{}へ保存: {name} / {}",
                    slot + 1,
                    current.as_deref().unwrap_or("範囲なし")
                ),
                self.name.is_some() && self.area.is_some(),
            );
            add(
                load,
                format!(
                    "枠{}を適用 → {name}: {}",
                    slot + 1,
                    self.presets[slot]
                        .map(dimensions)
                        .unwrap_or_else(|| "未保存".into())
                ),
                self.name.is_some() && self.presets[slot].is_some(),
            );
        }
        controls
    }
}

pub(super) fn resolve_control(
    action: AreaEditAction,
    revision: u64,
    epoch: u64,
    state: &AreaEditUiState,
    current: &AreaUiSnapshot,
) -> Option<InputAction> {
    if revision != state.revision
        || epoch != current.flags.epoch
        || state.snapshot.as_ref() != Some(current)
        || !current
            .controls()
            .iter()
            .any(|control| control.action == action && control.enabled)
    {
        return None;
    }
    Some(match action {
        AreaEditAction::Undo => InputAction::AreaUndo,
        AreaEditAction::Redo => InputAction::AreaRedo,
        AreaEditAction::Copy => InputAction::AreaCopy,
        AreaEditAction::Paste => InputAction::AreaPaste,
        AreaEditAction::Save1 => InputAction::AreaSavePreset1,
        AreaEditAction::Save2 => InputAction::AreaSavePreset2,
        AreaEditAction::Save3 => InputAction::AreaSavePreset3,
        AreaEditAction::Load1 => InputAction::AreaLoadPreset1,
        AreaEditAction::Load2 => InputAction::AreaLoadPreset2,
        AreaEditAction::Load3 => InputAction::AreaLoadPreset3,
    })
}

#[derive(SystemParam)]
pub struct AreaPresentationData<'w, 's> {
    selected: Res<'w, SelectedEntity>,
    history: Res<'w, AreaEditHistory>,
    clipboard: Res<'w, AreaEditClipboard>,
    presets: Res<'w, AreaEditPresets>,
    familiars: Query<'w, 's, &'static Familiar>,
    areas: Query<'w, 's, &'static TaskArea, With<Familiar>>,
}

pub fn update_area_edit_controls(
    data: AreaPresentationData,
    guard: AreaUiGuard,
    mut state: ResMut<AreaEditUiState>,
    mut model: ResMut<AreaEditPanelModel>,
) {
    let current = snapshot(
        data.selected.0,
        &data.history,
        &data.clipboard,
        &data.presets,
        (&data.familiars, &data.areas),
        guard.flags(),
    );
    if state.snapshot.as_ref() == Some(&current) {
        return;
    }
    state.revision = state.revision.wrapping_add(1);
    *model = AreaEditPanelModel {
        visible: current.flags.visible,
        revision: state.revision,
        epoch: current.flags.epoch,
        controls: current.controls(),
    };
    state.snapshot = Some(current);
}
