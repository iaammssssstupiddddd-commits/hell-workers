use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct EntityListDirty {
    structure_dirty: bool,
    value_dirty: bool,
}

impl EntityListDirty {
    pub fn mark_structure(&mut self) {
        self.structure_dirty = true;
    }

    pub fn mark_values(&mut self) {
        self.value_dirty = true;
    }

    pub fn clear_all(&mut self) {
        self.structure_dirty = false;
        self.value_dirty = false;
    }

    pub fn clear_values(&mut self) {
        self.value_dirty = false;
    }

    pub fn needs_structure_sync(&self) -> bool {
        self.structure_dirty
    }

    pub fn needs_value_sync_only(&self) -> bool {
        self.value_dirty && !self.structure_dirty
    }
}
