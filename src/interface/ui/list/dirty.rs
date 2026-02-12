use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct EntityListDirty(bool);

impl EntityListDirty {
    pub fn mark(&mut self) {
        self.0 = true;
    }

    pub fn clear(&mut self) {
        self.0 = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.0
    }
}
