//! エンティティリスト検索状態

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct EntityListSearchState {
    pub query: String,
    pub last_applied: String,
}

impl EntityListSearchState {
    pub fn normalized(&self) -> &str {
        self.query.trim()
    }
}
