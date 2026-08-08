//! MudMixer auto-haul system

use super::mixer_helpers;

use bevy::prelude::*;

use hw_core::relationships::TaskWorkers;
use hw_jobs::mud_mixer::{MudMixerStorage, TargetMixer};

use crate::resource_cache::SharedResourceCache;
use crate::transport_request::TransportRequest;
use crate::transport_request::producer::active_unit_cache::{
    CachedActiveFamiliars, CachedActiveYards,
};
use crate::types::ResourceType;

type MixerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static MudMixerStorage,
        Option<&'static TaskWorkers>,
        Option<&'static hw_jobs::MovePlanned>,
        Option<&'static hw_jobs::DeconstructionPending>,
    ),
>;

type MixerRequestQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TargetMixer,
        &'static TransportRequest,
        Option<&'static hw_jobs::Designation>,
        Option<&'static TaskWorkers>,
    ),
>;

pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    haul_cache: Res<SharedResourceCache>,
    familiars_cache: Res<CachedActiveFamiliars>,
    yards_cache: Res<CachedActiveYards>,
    q_mixers: MixerQuery,
    q_mixer_requests: MixerRequestQuery,
    q_stockpiles_detailed: mixer_helpers::StockpilesDetailedQuery,
) {
    let active_familiars = &familiars_cache.data;
    let active_yards = &yards_cache.data;
    let all_owners = super::collect_all_area_owners(active_familiars, active_yards);

    let (water_inflight_by_mixer, sand_inflight_by_mixer) =
        mixer_helpers::collect_inflight_mixer_requests(&q_mixer_requests);

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();
    let mut active_mixers = std::collections::HashSet::<Entity>::new();

    mixer_helpers::compute_mixer_desired_requests(
        &q_mixers,
        &mut desired_requests,
        &mut active_mixers,
        &all_owners,
        active_yards,
        &q_stockpiles_detailed,
        mixer_helpers::MixerInflightContext {
            haul_cache: &haul_cache,
            water_inflight_by_mixer: &water_inflight_by_mixer,
            sand_inflight_by_mixer: &sand_inflight_by_mixer,
        },
    );

    mixer_helpers::upsert_mixer_requests(
        &mut commands,
        &q_mixer_requests,
        &desired_requests,
        &active_mixers,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport_request::producer::active_unit_cache::{
        CachedActiveFamiliars, CachedActiveYards,
    };
    use crate::zone::Stockpile;
    use hw_jobs::DeconstructionPending;
    use hw_world::Yard;

    #[test]
    fn pending_mixer_produces_no_delivery_requests_while_live_mixer_does() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SharedResourceCache>()
            .init_resource::<CachedActiveFamiliars>()
            .init_resource::<CachedActiveYards>()
            .add_systems(Update, mud_mixer_auto_haul_system);
        let yard_entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<CachedActiveYards>()
            .data
            .push((
                yard_entity,
                Yard {
                    min: Vec2::splat(-100.0),
                    max: Vec2::splat(100.0),
                },
            ));
        let spawn_mixer = |app: &mut App, x: f32| {
            app.world_mut()
                .spawn((
                    Transform::from_xyz(x, 0.0, 0.0),
                    MudMixerStorage::default(),
                    Stockpile {
                        capacity: 8,
                        resource_type: Some(ResourceType::Water),
                    },
                ))
                .id()
        };
        let pending = spawn_mixer(&mut app, 0.0);
        let live = spawn_mixer(&mut app, 20.0);
        let order = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(pending)
            .insert(DeconstructionPending { order });

        app.update();

        let mut requests = app.world_mut().query::<&TransportRequest>();
        let anchors = requests
            .iter(app.world())
            .map(|request| request.anchor)
            .collect::<Vec<_>>();
        assert!(!anchors.contains(&pending));
        assert!(anchors.contains(&live));
    }
}
