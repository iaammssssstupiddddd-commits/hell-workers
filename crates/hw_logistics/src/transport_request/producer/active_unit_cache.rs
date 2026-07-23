//! アクティブ Familiar / Yard のフレームキャッシュ
//!
//! Perceive フェーズで 1 回だけ構築し、同フレームの全 producer が共有する。
//! これにより、producer ごとにアクティブリストを繰り返し構築する O(n × producer数) を
//! O(n) × 1 回 に削減する。

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_spatial::StockpileSpatialGrid;
use hw_world::zones::{AreaBounds, Yard};

use crate::transport_request::producer::stockpile_group::{
    StockpileGroup, StockpileGroupSpatialIndex, build_group_spatial_index, build_stockpile_groups,
};
use crate::zone::{Stockpile, StockpilePolicy};

type StockpilesQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<Stockpile>, With<StockpilePolicy>)>;

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

/// Stockpile の構造的な group membership と空間インデックス。
///
/// policy 値、内容量、搬入予約の動的集計は保持せず、producer が live component から読む。
#[derive(Resource, Default)]
pub struct CachedStockpileGroups {
    pub groups: Vec<StockpileGroup>,
    pub spatial_index: StockpileGroupSpatialIndex,
    initialized: bool,
    generation: u64,
}

impl CachedStockpileGroups {
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
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
    q_added_policies: Query<(), Added<StockpilePolicy>>,
    mut removed_policies: RemovedComponents<StockpilePolicy>,
    mut cache: ResMut<CachedStockpileGroups>,
) {
    let policy_membership_changed =
        !q_added_policies.is_empty() || removed_policies.read().count() != 0;
    if cache.initialized
        && !stockpile_grid.is_changed()
        && !yards_cache.is_changed()
        && !policy_membership_changed
    {
        return;
    }

    let active_yards = &yards_cache.data;
    cache.groups = build_stockpile_groups(&stockpile_grid, active_yards, &q_stockpiles);
    cache.spatial_index = build_group_spatial_index(&cache.groups, active_yards);
    cache.initialized = true;
    cache.generation = cache.generation.wrapping_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport_request::TransportPriority;
    use crate::zone::StockpileAcceptance;
    use hw_spatial::SpatialGridOps;

    #[test]
    fn group_cache_tracks_membership_but_not_live_policy_values() {
        let mut app = App::new();
        app.init_resource::<StockpileSpatialGrid>()
            .init_resource::<CachedActiveYards>()
            .init_resource::<CachedStockpileGroups>()
            .add_systems(Update, update_cached_stockpile_groups_system);

        let yard_entity = app.world_mut().spawn_empty().id();
        let yard = Yard {
            min: Vec2::splat(-16.0),
            max: Vec2::splat(16.0),
        };
        {
            let mut yards = app.world_mut().resource_mut::<CachedActiveYards>();
            yards.data.push((yard_entity, yard));
            yards.initialized = true;
        }

        let managed = app
            .world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity: 4,
                    resource_type: None,
                },
                StockpilePolicy::for_capacity(4),
            ))
            .id();
        let unmanaged_special = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 0.0, 0.0),
                Stockpile {
                    capacity: 4,
                    resource_type: None,
                },
            ))
            .id();
        {
            let mut grid = app.world_mut().resource_mut::<StockpileSpatialGrid>();
            grid.insert(managed, Vec2::ZERO);
            grid.insert(unmanaged_special, Vec2::new(1.0, 0.0));
        }

        app.update();
        let first_generation = app.world().resource::<CachedStockpileGroups>().generation();
        assert_eq!(first_generation, 1);
        assert_eq!(
            app.world().resource::<CachedStockpileGroups>().groups[0].cells,
            vec![managed]
        );

        app.update();
        assert_eq!(
            app.world().resource::<CachedStockpileGroups>().generation(),
            first_generation
        );

        app.world_mut().entity_mut(managed).insert(StockpilePolicy {
            acceptance: StockpileAcceptance::Any,
            inbound_priority: TransportPriority::Critical,
            target_amount: 2,
            allow_export: false,
        });
        app.update();
        assert_eq!(
            app.world().resource::<CachedStockpileGroups>().generation(),
            first_generation
        );

        app.world_mut()
            .entity_mut(managed)
            .remove::<StockpilePolicy>();
        app.update();
        let cache = app.world().resource::<CachedStockpileGroups>();
        assert_eq!(cache.generation(), first_generation + 1);
        assert!(cache.groups.is_empty());
    }
}
