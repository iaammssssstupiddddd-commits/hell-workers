use crate::systems::command::{AreaEditHandleKind, TaskArea};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(super) enum AreaEditOperation {
    Move,
    Resize(AreaEditHandleKind),
}

#[derive(Clone)]
pub(super) struct AreaEditDrag {
    pub(super) familiar_entity: Entity,
    pub(super) operation: AreaEditOperation,
    pub(super) original_area: TaskArea,
    pub(super) drag_start: Vec2,
}

#[derive(Resource, Default)]
pub struct AreaEditSession {
    pub(super) active_drag: Option<AreaEditDrag>,
    /// Dream植林の保留リクエスト（DreamPlantingモードでドラッグ確定時にセット）
    pub pending_dream_planting: Option<(Vec2, Vec2)>,
}

impl AreaEditSession {
    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    pub fn operation_label(&self) -> Option<&'static str> {
        let drag = self.active_drag.as_ref()?;
        Some(match drag.operation {
            AreaEditOperation::Move => "Move",
            AreaEditOperation::Resize(AreaEditHandleKind::TopLeft) => "Resize TL",
            AreaEditOperation::Resize(AreaEditHandleKind::Top) => "Resize T",
            AreaEditOperation::Resize(AreaEditHandleKind::TopRight) => "Resize TR",
            AreaEditOperation::Resize(AreaEditHandleKind::Right) => "Resize R",
            AreaEditOperation::Resize(AreaEditHandleKind::BottomRight) => "Resize BR",
            AreaEditOperation::Resize(AreaEditHandleKind::Bottom) => "Resize B",
            AreaEditOperation::Resize(AreaEditHandleKind::BottomLeft) => "Resize BL",
            AreaEditOperation::Resize(AreaEditHandleKind::Left) => "Resize L",
            AreaEditOperation::Resize(AreaEditHandleKind::Center) => "Move",
        })
    }
}

#[derive(Clone)]
pub(super) struct AreaEditHistoryEntry {
    pub(super) familiar_entity: Entity,
    pub(super) before: Option<TaskArea>,
    pub(super) after: Option<TaskArea>,
}

#[derive(Resource, Default)]
pub struct AreaEditHistory {
    pub(super) undo_stack: Vec<AreaEditHistoryEntry>,
    pub(super) redo_stack: Vec<AreaEditHistoryEntry>,
}

impl AreaEditHistory {
    pub fn push(
        &mut self,
        familiar_entity: Entity,
        before: Option<TaskArea>,
        after: Option<TaskArea>,
    ) {
        if before.as_ref().map(|a| (a.min, a.max)) == after.as_ref().map(|a| (a.min, a.max)) {
            return;
        }

        const MAX_HISTORY: usize = 64;
        self.undo_stack.push(AreaEditHistoryEntry {
            familiar_entity,
            before,
            after,
        });
        if self.undo_stack.len() > MAX_HISTORY {
            let drop_count = self.undo_stack.len() - MAX_HISTORY;
            self.undo_stack.drain(0..drop_count);
        }
        self.redo_stack.clear();
    }
}

#[derive(Resource, Default)]
pub struct AreaEditClipboard {
    pub(super) area: Option<TaskArea>,
}

impl AreaEditClipboard {
    pub fn has_area(&self) -> bool {
        self.area.is_some()
    }
}

#[derive(Resource, Default)]
pub struct AreaEditPresets {
    slots: [Option<Vec2>; 3],
}

impl AreaEditPresets {
    pub fn save_size(&mut self, slot: usize, size: Vec2) {
        if slot < self.slots.len() {
            self.slots[slot] = Some(size.abs());
        }
    }

    pub fn get_size(&self, slot: usize) -> Option<Vec2> {
        self.slots.get(slot).and_then(|size| *size)
    }
}

pub(super) use AreaEditDrag as Drag;
pub(super) use AreaEditOperation as Operation;
