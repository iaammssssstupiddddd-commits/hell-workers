/// タスクリストの1エントリ
#[derive(Clone, PartialEq)]
pub struct TaskEntry {
    pub entity: bevy::prelude::Entity,
    pub description: String,
    pub priority: u32,
    pub worker_count: usize,
}

/// タスクリストの汚れフラグ Resource
#[derive(bevy::prelude::Resource, Default)]
pub struct TaskListDirty {
    state_dirty: bool,
    list_dirty: bool,
    summary_dirty: bool,
}

impl TaskListDirty {
    pub fn mark_all(&mut self) {
        self.state_dirty = true;
        self.list_dirty = true;
        self.summary_dirty = true;
    }

    pub fn mark_state(&mut self) {
        self.state_dirty = true;
    }

    pub fn mark_summary(&mut self) {
        self.summary_dirty = true;
    }

    pub fn mark_list(&mut self) {
        self.list_dirty = true;
    }

    pub fn clear_list(&mut self) {
        self.list_dirty = false;
    }

    pub fn clear_state(&mut self) {
        self.state_dirty = false;
    }

    pub fn clear_summary(&mut self) {
        self.summary_dirty = false;
    }

    pub fn state_dirty(&self) -> bool {
        self.state_dirty
    }

    pub fn list_dirty(&self) -> bool {
        self.list_dirty
    }

    pub fn summary_dirty(&self) -> bool {
        self.summary_dirty
    }
}
