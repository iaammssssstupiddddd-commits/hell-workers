//! TransportRequest のライフサイクル管理
//!
//! Maintain フェーズ: アンカー消失時のリクエスト cleanup

use super::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportRequest, TransportRequestFixedSource,
};
use bevy::prelude::*;
use hw_core::events::SoulTaskUnassignRequest;
use hw_core::familiar::Familiar;
use hw_core::relationships::{ManagedBy, StoredIn, TaskWorkers};
use hw_jobs::{Designation, Priority, TaskSlots};
use hw_world::zones::Yard;

use crate::types::ResourceItem;

/// Result of requesting closure through the manual-transport lifecycle owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualTransportCloseResult {
    /// The request and all owner-managed runtime state were scheduled for cleanup.
    Closed,
    /// The request was malformed (no fixed source), but was still safely closed.
    MalformedClosed,
    /// The target is not a manual transport request and was left untouched.
    Unsupported,
}

/// Live components required by the owner-controlled close primitive.
///
/// Callers may cache display capabilities, but must construct this value again
/// from the current world before applying a user action.
pub struct ManualTransportCloseContext<'a> {
    pub request_entity: Entity,
    pub manual: Option<&'a ManualTransportRequest>,
    pub fixed_source: Option<&'a TransportRequestFixedSource>,
    pub workers: Option<&'a TaskWorkers>,
    pub resource_item: Option<&'a ResourceItem>,
}

/// Close a manual transport request through its owning crate.
///
/// This is the only public path that removes the pinned source and request
/// components. Worker cleanup remains owned by Soul AI and is requested through
/// `SoulTaskUnassignRequest`.
pub fn close_manual_transport_request(
    commands: &mut Commands,
    context: ManualTransportCloseContext<'_>,
) -> ManualTransportCloseResult {
    if context.manual.is_none() {
        return ManualTransportCloseResult::Unsupported;
    }

    let result = if context.fixed_source.is_some() {
        ManualTransportCloseResult::Closed
    } else {
        ManualTransportCloseResult::MalformedClosed
    };
    close_transport_request(
        commands,
        context.request_entity,
        context.fixed_source,
        context.workers,
        context.resource_item,
    );
    result
}

fn close_transport_request(
    commands: &mut Commands,
    request_entity: Entity,
    fixed_source: Option<&TransportRequestFixedSource>,
    workers: Option<&TaskWorkers>,
    resource_item: Option<&ResourceItem>,
) {
    if let Some(workers) = workers {
        for &soul_entity in workers.iter() {
            commands.write_message(SoulTaskUnassignRequest {
                soul_entity,
                emit_abandoned: true,
            });
        }
    }

    if let Some(source) = fixed_source {
        commands
            .entity(source.0)
            .try_remove::<ManualHaulPinnedSource>();
    }

    if resource_item.is_some() {
        commands.entity(request_entity).try_remove::<(
            TransportRequest,
            Designation,
            super::TransportDemand,
            super::TransportPolicy,
            super::TransportRequestState,
            super::WheelbarrowLease,
            super::WheelbarrowPendingSince,
            ManualTransportRequest,
            TransportRequestFixedSource,
            TaskSlots,
            ManagedBy,
            Priority,
        )>();
    } else {
        commands.entity(request_entity).try_despawn();
    }
}

type AnchorCleanupRequestQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TransportRequest,
        Option<&'static super::TransportDemand>,
        Option<&'static TaskWorkers>,
        Option<&'static ManualTransportRequest>,
        Option<&'static TransportRequestFixedSource>,
    ),
>;

/// アンカー（搬入先）が消失した request を close する
pub fn transport_request_anchor_cleanup_system(
    mut commands: Commands,
    q_requests: AnchorCleanupRequestQuery,
    q_any: Query<Entity>,
    q_items: Query<&ResourceItem>,
    q_familiars: Query<Entity, With<Familiar>>,
    q_yards: Query<Entity, With<Yard>>,
    q_in_stockpile: Query<(), With<StoredIn>>,
) {
    for (request_entity, req, demand_opt, workers_opt, manual_opt, fixed_source_opt) in
        q_requests.iter()
    {
        let mut should_close = false;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        // 1. アンカー（搬入先）が消失した
        if q_any.get(req.anchor).is_err() {
            should_close = true;
        }

        // 2. 需要がゼロになり、かつ作業中のワーカーもいない
        if let Some(demand) = demand_opt
            && demand.desired_slots == 0
            && workers == 0
        {
            should_close = true;
        }

        // 3. 発行者（Familiar/Yard）が消失した
        if q_any.get(req.issued_by).is_err()
            || (q_familiars.get(req.issued_by).is_err() && q_yards.get(req.issued_by).is_err())
        {
            should_close = true;
        }

        // 4. manual request の固定 source が消失/搬送済みなら close
        if manual_opt.is_some() {
            match fixed_source_opt {
                Some(source) => {
                    if q_any.get(source.0).is_err() {
                        should_close = true;
                    }
                    if workers == 0 && q_in_stockpile.get(source.0).is_ok() {
                        should_close = true;
                    }
                }
                None => {
                    should_close = true;
                }
            }
        }

        if should_close {
            let resource_item = q_items.get(request_entity).ok();
            if manual_opt.is_some() {
                close_manual_transport_request(
                    &mut commands,
                    ManualTransportCloseContext {
                        request_entity,
                        manual: manual_opt,
                        fixed_source: fixed_source_opt,
                        workers: workers_opt,
                        resource_item,
                    },
                );
            } else {
                close_transport_request(
                    &mut commands,
                    request_entity,
                    fixed_source_opt,
                    workers_opt,
                    resource_item,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_core::relationships::WorkingOn;

    #[derive(Resource)]
    struct CloseTarget(Entity);

    type ManualCloseTestQuery<'w, 's> = Query<
        'w,
        's,
        (
            Option<&'static ManualTransportRequest>,
            Option<&'static TransportRequestFixedSource>,
            Option<&'static TaskWorkers>,
            Option<&'static ResourceItem>,
        ),
    >;

    fn close_manual(
        mut commands: Commands,
        target: Res<CloseTarget>,
        requests: ManualCloseTestQuery,
    ) {
        let (manual, fixed_source, workers, resource_item) =
            requests.get(target.0).expect("request must exist");
        assert_eq!(
            close_manual_transport_request(
                &mut commands,
                ManualTransportCloseContext {
                    request_entity: target.0,
                    manual,
                    fixed_source,
                    workers,
                    resource_item,
                },
            ),
            ManualTransportCloseResult::Closed
        );
    }

    #[test]
    fn transport_request_lifecycle_manual_close_unpins_and_requests_worker_cleanup() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SoulTaskUnassignRequest>()
            .add_systems(Update, (close_manual, ApplyDeferred).chain());

        let source = app.world_mut().spawn(ManualHaulPinnedSource).id();
        let request = app
            .world_mut()
            .spawn((ManualTransportRequest, TransportRequestFixedSource(source)))
            .id();
        let soul = app.world_mut().spawn(WorkingOn(request)).id();
        app.insert_resource(CloseTarget(request));

        app.update();

        assert!(app.world().get_entity(request).is_err());
        assert!(app.world().get::<ManualHaulPinnedSource>(source).is_none());
        let messages = app.world().resource::<Messages<SoulTaskUnassignRequest>>();
        assert_eq!(messages.len(), 1);
        assert!(
            messages
                .iter_current_update_messages()
                .any(|message| { message.soul_entity == soul && message.emit_abandoned })
        );
    }
}
