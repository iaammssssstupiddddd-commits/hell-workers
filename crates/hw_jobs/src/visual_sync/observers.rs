//! ECS ライフサイクル Observer — コンポーネントの追加・削除イベントに応じて
//! visual mirror コンポーネントを付与・除去する。

use bevy::ecs::lifecycle::{Add, Remove};
use bevy::prelude::*;

use hw_core::visual_mirror::building::{BuildingVisualState, MudMixerVisualState};
use hw_core::visual_mirror::gather::{GatherHighlightMarker, RestAreaVisual};

use crate::model::{Building, Designation, RestArea, Rock, Tree};
use crate::mud_mixer::MudMixerStorage;

use super::building_type_to_visual;

pub fn on_designation_added(
    on: On<Add, Designation>,
    mut commands: Commands,
    q: Query<(), Or<(With<Tree>, With<Rock>)>>,
) {
    if q.contains(on.entity) {
        commands.entity(on.entity).try_insert(GatherHighlightMarker);
    }
}

pub fn on_designation_removed(on: On<Remove, Designation>, mut commands: Commands) {
    commands.entity(on.entity).remove::<GatherHighlightMarker>();
}

pub fn on_rest_area_added(on: On<Add, RestArea>, mut commands: Commands, q: Query<&RestArea>) {
    if let Ok(rest_area) = q.get(on.entity) {
        commands.entity(on.entity).try_insert(RestAreaVisual {
            capacity: rest_area.capacity,
        });
    }
}

/// Inserts `BuildingVisualState` when a `Building` component is added.
pub fn on_building_added_sync_visual(
    on: On<Add, Building>,
    mut commands: Commands,
    q: Query<&Building>,
) {
    if let Ok(building) = q.get(on.entity) {
        commands.entity(on.entity).try_insert(BuildingVisualState {
            kind: building_type_to_visual(building.kind),
            is_provisional: building.is_provisional,
        });
    }
}

/// Inserts `MudMixerVisualState` when a `MudMixerStorage` component is added.
pub fn on_mud_mixer_storage_added(on: On<Add, MudMixerStorage>, mut commands: Commands) {
    commands.entity(on.entity).try_insert(MudMixerVisualState::default());
}
