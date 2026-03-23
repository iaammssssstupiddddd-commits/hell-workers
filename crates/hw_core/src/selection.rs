use bevy::prelude::*;

/// 現在選択中のエンティティ
#[derive(Resource, Default)]
pub struct SelectedEntity(pub Option<Entity>);

/// 現在ホバー中のエンティティ
#[derive(Resource, Default)]
pub struct HoveredEntity(pub Option<Entity>);

/// 選択ハイライト表示エンティティのマーカー
#[derive(Component)]
pub struct SelectionIndicator;
