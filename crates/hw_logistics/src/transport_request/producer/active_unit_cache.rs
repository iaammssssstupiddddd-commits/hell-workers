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

/// アクティブな Familiar（Idle 以外）のリスト。毎フレーム Perceive 時に更新される。
#[derive(Resource, Default)]
pub struct CachedActiveFamiliars {
    pub data: Vec<(Entity, AreaBounds)>,
}

/// 全 Yard エンティティのリスト。毎フレーム Perceive 時に更新される。
#[derive(Resource, Default)]
pub struct CachedActiveYards {
    pub data: Vec<(Entity, Yard)>,
}

/// Stockpile グループと空間インデックス。毎フレーム Perceive 時に 1 回だけ構築される。
#[derive(Resource, Default)]
pub struct CachedStockpileGroups {
    pub groups: Vec<StockpileGroup>,
    pub spatial_index: StockpileGroupSpatialIndex,
}

/// CachedActiveFamiliars を更新する。Idle の Familiar は除外する。
pub fn update_cached_active_familiars_system(
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    mut cache: ResMut<CachedActiveFamiliars>,
) {
    cache.data.clear();
    cache.data.extend(
        q_familiars
            .iter()
            .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
            .map(|(entity, _, area)| (entity, area.bounds())),
    );
}

/// CachedActiveYards を更新する。全 Yard エンティティを収集する。
pub fn update_cached_active_yards_system(
    q_yards: Query<(Entity, &Yard)>,
    mut cache: ResMut<CachedActiveYards>,
) {
    cache.data.clear();
    cache.data.extend(q_yards.iter().map(|(e, y)| (e, y.clone())));
}

/// Stockpile グループと空間インデックスを 1 回だけ構築する。
pub fn update_cached_stockpile_groups_system(
    stockpile_grid: Res<StockpileSpatialGrid>,
    yards_cache: Res<CachedActiveYards>,
    q_stockpiles: StockpilesQuery,
    mut cache: ResMut<CachedStockpileGroups>,
) {
    let active_yards = &yards_cache.data;
    cache.groups = build_stockpile_groups(&stockpile_grid, active_yards, &q_stockpiles);
    cache.spatial_index = build_group_spatial_index(&cache.groups, active_yards);
}

