use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct SelectedEntity(pub Option<Entity>);

#[derive(Resource, Default)]
pub struct HoveredEntity(pub Option<Entity>);

#[derive(Component)]
pub struct SelectionIndicator;
