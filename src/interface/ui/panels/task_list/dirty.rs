use bevy::prelude::Resource;

#[derive(Resource, Default)]
pub struct TaskListDirty {
    list_dirty: bool,
    summary_dirty: bool,
}

impl TaskListDirty {
    pub fn mark_all(&mut self) {
        self.list_dirty = true;
        self.summary_dirty = true;
    }

    pub fn mark_list(&mut self) {
        self.list_dirty = true;
    }

    pub fn clear_list(&mut self) {
        self.list_dirty = false;
    }

    pub fn clear_summary(&mut self) {
        self.summary_dirty = false;
    }

    pub fn list_dirty(&self) -> bool {
        self.list_dirty
    }

    pub fn summary_dirty(&self) -> bool {
        self.summary_dirty
    }
}
