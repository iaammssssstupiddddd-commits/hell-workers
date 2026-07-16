//! アクティブ Familiar / Yard のフレームキャッシュ
//!
//! Perceive フェーズで 1 回だけ構築し、同フレームの全 producer が共有する。
//! これにより、producer ごとにアクティブリストを繰り返し構築する O(n × producer数) を
//! O(n) × 1 回 に削減する。

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::StoredItems;
use hw_spatial::StockpileSpatialGrid;
use hw_world::zones::{AreaBounds, Yard};

use crate::transport_request::producer::stockpile_group::{
    StockpileGroup, StockpileGroupSpatialIndex, build_group_spatial_index, build_stockpile_groups,
};
use crate::types::BucketStorage;
use crate::zone::Stockpile;

type StockpilesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Stockpile,
        Option<&'static StoredItems>,
        Option<&'static BucketStorage>,
    ),
>;

type ActiveFamiliarDirtyQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<hw_core::familiar::Familiar>,
        Or<(
            Added<hw_core::familiar::Familiar>,
            Changed<hw_core::familiar::Familiar>,
            Added<ActiveCommand>,
            Changed<ActiveCommand>,
            Added<TaskArea>,
            Changed<TaskArea>,
        )>,
    ),
>;

type ActiveYardDirtyQuery<'w, 's> = Query<'w, 's, (), Or<(Added<Yard>, Changed<Yard>)>>;

/// アクティブな Familiar（Idle 以外）のリスト。
///
/// Entity を保持するため load reset で既定値へ戻す。通常時は source component
/// の Added / Changed / Removed がない限り再構築しない。
#[derive(Resource, Default)]
pub struct CachedActiveFamiliars {
    pub data: Vec<(Entity, AreaBounds)>,
    initialized: bool,
}

/// 全 Yard エンティティのリスト。変更時だけ再構築する。
#[derive(Resource, Default)]
pub struct CachedActiveYards {
    pub data: Vec<(Entity, Yard)>,
    initialized: bool,
}

/// Stockpile グループと空間インデックス。毎フレーム Perceive 時に 1 回だけ構築される。
#[derive(Resource, Default)]
pub struct CachedStockpileGroups {
    pub groups: Vec<StockpileGroup>,
    pub spatial_index: StockpileGroupSpatialIndex,
    initialized: bool,
}

/// CachedActiveFamiliars を更新する。Idle の Familiar は除外する。
///
/// producer はこの resource を読み取るだけなので、steady-state では resource
/// 自身も Changed にならず、下流の stockpile group cache まで再構築しない。
pub fn update_cached_active_familiars_system(
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea), With<hw_core::familiar::Familiar>>,
    q_dirty: ActiveFamiliarDirtyQuery,
    mut removed_familiars: RemovedComponents<hw_core::familiar::Familiar>,
    mut removed_commands: RemovedComponents<ActiveCommand>,
    mut removed_task_areas: RemovedComponents<TaskArea>,
    mut cache: ResMut<CachedActiveFamiliars>,
) {
    let changed = !cache.initialized
        || !q_dirty.is_empty()
        || removed_familiars.read().count() != 0
        || removed_commands.read().count() != 0
        || removed_task_areas.read().count() != 0;
    if !changed {
        return;
    }

    cache.data.clear();
    cache.data.extend(
        q_familiars
            .iter()
            .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
            .map(|(entity, _, area)| (entity, area.bounds())),
    );
    cache.initialized = true;
}

/// CachedActiveYards を更新する。全 Yard エンティティを収集する。
pub fn update_cached_active_yards_system(
    q_yards: Query<(Entity, &Yard)>,
    q_dirty: ActiveYardDirtyQuery,
    mut removed_yards: RemovedComponents<Yard>,
    mut cache: ResMut<CachedActiveYards>,
) {
    let changed = !cache.initialized || !q_dirty.is_empty() || removed_yards.read().count() != 0;
    if !changed {
        return;
    }

    cache.data.clear();
    cache
        .data
        .extend(q_yards.iter().map(|(e, y)| (e, y.clone())));
    cache.initialized = true;
}

/// Stockpile グループと空間インデックスを 1 回だけ構築する。
pub fn update_cached_stockpile_groups_system(
    stockpile_grid: Res<StockpileSpatialGrid>,
    yards_cache: Res<CachedActiveYards>,
    q_stockpiles: StockpilesQuery,
    mut cache: ResMut<CachedStockpileGroups>,
) {
    if cache.initialized && !stockpile_grid.is_changed() && !yards_cache.is_changed() {
        return;
    }

    let active_yards = &yards_cache.data;
    cache.groups = build_stockpile_groups(&stockpile_grid, active_yards, &q_stockpiles);
    cache.spatial_index = build_group_spatial_index(&cache.groups, active_yards);
    cache.initialized = true;
}
