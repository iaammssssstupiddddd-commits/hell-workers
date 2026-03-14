use bevy::prelude::*;
use hw_core::area::TaskArea;

/// Which handle of a resize/move gizmo was interacted with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaEditHandleKind {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Center,
}

#[derive(Clone, Copy, Debug)]
pub enum AreaEditOperation {
    Move,
    Resize(AreaEditHandleKind),
}

#[derive(Clone)]
pub struct AreaEditDrag {
    pub familiar_entity: Entity,
    pub operation: AreaEditOperation,
    pub original_area: TaskArea,
    pub drag_start: Vec2,
}

#[derive(Resource, Default)]
pub struct AreaEditSession {
    pub active_drag: Option<AreaEditDrag>,
    /// Dream植林の保留リクエスト（DreamPlantingモードでドラッグ確定時にセット）
    pub pending_dream_planting: Option<(Vec2, Vec2, u64)>,
    /// Dream植林ドラッグ中に使う固定シード（プレビューと確定結果を一致させる）
    pub dream_planting_preview_seed: Option<u64>,
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
pub struct AreaEditHistoryEntry {
    pub familiar_entity: Entity,
    pub before: Option<TaskArea>,
    pub after: Option<TaskArea>,
}

#[derive(Resource, Default)]
pub struct AreaEditHistory {
    pub undo_stack: Vec<AreaEditHistoryEntry>,
    pub redo_stack: Vec<AreaEditHistoryEntry>,
}

impl AreaEditHistory {
    pub fn push(
        &mut self,
        familiar_entity: Entity,
        before: Option<TaskArea>,
        after: Option<TaskArea>,
    ) {
        if before.as_ref().map(|a| (a.min(), a.max()))
            == after.as_ref().map(|a| (a.min(), a.max()))
        {
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
    pub area: Option<TaskArea>,
}

impl AreaEditClipboard {
    pub fn has_area(&self) -> bool {
        self.area.is_some()
    }
}

#[derive(Resource, Default)]
pub struct AreaEditPresets {
    pub slots: [Option<Vec2>; 3],
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
