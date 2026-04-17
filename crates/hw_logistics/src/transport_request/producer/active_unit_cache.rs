//! アクティブ Familiar / Yard のフレームキャッシュ
//!
//! Perceive フェーズで 1 回だけ構築し、同フレームの全 producer が共有する。
//! これにより、producer ごとにアクティブリストを繰り返し構築する O(n × producer数) を
//! O(n) × 1 回 に削減する。

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_world::zones::{AreaBounds, Yard};

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
