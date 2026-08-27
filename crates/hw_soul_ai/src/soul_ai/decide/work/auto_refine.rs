//! Auto-refine system for MudMixer
//!
//! Automatically creates refine tasks when materials are ready in MudMixer.

use bevy::prelude::*;
use std::collections::HashSet;

use hw_core::area::TaskArea;
use hw_core::constants::MUD_MIXER_REFINE_PRIORITY;
use hw_core::events::{DesignationOp, DesignationRequest};
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::logistics::ResourceType;
use hw_core::relationships::{StoredItems, TaskWorkers};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{AssignedTask, DeconstructionPending, Designation, MovePlanned, RefineActivityIndex};
use hw_logistics::transport_request::producer::{collect_all_area_owners, find_owner_for_position};
use hw_logistics::zone::Stockpile;
use hw_world::Yard;

type MixersQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static MudMixerStorage,
        Option<&'static TaskWorkers>,
        Option<&'static Designation>,
        Option<&'static Stockpile>,
        Option<&'static StoredItems>,
        Option<&'static MovePlanned>,
        Option<&'static DeconstructionPending>,
    ),
>;

/// MudMixer で精製タスクを自動発行するシステム
pub fn mud_mixer_auto_refine_system(
    mut designation_writer: MessageWriter<DesignationRequest>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_mixers: MixersQuery,
    q_souls: Query<&AssignedTask>,
    activity_index: Option<Res<RefineActivityIndex>>,
    mut bootstrap_in_flight: Local<HashSet<Entity>>,
) {
    // The shared index is synchronized after Execute and is therefore the
    // previous frame's settled snapshot here. Before its first sync (including
    // isolated tests), retain the old full-scan behavior as a fail-closed boot lane.
    let activity_index = activity_index.filter(|index| index.is_initialized());
    bootstrap_in_flight.clear();
    if activity_index.is_none() {
        for task in &q_souls {
            if let AssignedTask::Refine(data) = task {
                bootstrap_in_flight.insert(data.mixer);
            }
        }
    }

    // haul システムと同様に、非アイドル使い魔と Yard を組み合わせてオーナーを決定する
    let active_familiars: Vec<_> = q_familiars
        .iter()
        .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
        .map(|(e, _, area)| (e, area.bounds()))
        .collect();
    let active_yards: Vec<_> = q_yards.iter().map(|(e, y)| (e, y.clone())).collect();
    let all_owners = collect_all_area_owners(&active_familiars, &active_yards);

    for (
        mixer_entity,
        mixer_transform,
        storage,
        workers_opt,
        designation_opt,
        stockpile_opt,
        stored_opt,
        move_planned_opt,
        deconstruction_pending,
    ) in q_mixers.iter()
    {
        if move_planned_opt.is_some() || deconstruction_pending.is_some() {
            continue;
        }
        let mixer_pos = mixer_transform.translation.truncate();

        // オーナー（使い魔 or Yard）が存在するミキサーのみ対象
        let Some((owner_entity, _)) =
            find_owner_for_position(mixer_pos, &all_owners, &active_yards)
        else {
            continue;
        };

        // 既に Designation がある場合はスキップ
        if designation_opt.is_some() {
            continue;
        }

        // 原料が揃っているかチェック
        let water_count = match (stockpile_opt, stored_opt) {
            (Some(stockpile), Some(stored_items))
                if stockpile.resource_type == Some(ResourceType::Water) =>
            {
                stored_items.len() as u32
            }
            _ => 0,
        };

        if storage.has_materials_for_refining(water_count) {
            if !storage.has_output_capacity_for_refining() {
                continue;
            }
            let has_assigned_refine = activity_index.as_ref().map_or_else(
                || bootstrap_in_flight.contains(&mixer_entity),
                |index| index.assigned_count(mixer_entity) > 0,
            );
            let has_workers = workers_opt.is_some_and(|workers| !workers.is_empty());

            // 作業員が1名未満（精製は1人で行う）かつ、予約中のタスクがない場合
            if !has_workers && !has_assigned_refine {
                // Refine タスクを発行
                designation_writer.write(DesignationRequest {
                    entity: mixer_entity,
                    operation: DesignationOp::Issue {
                        work_type: hw_core::jobs::WorkType::Refine,
                        issued_by: owner_entity,
                        task_slots: 1,
                        priority: Some(MUD_MIXER_REFINE_PRIORITY),
                        target_blueprint: None,
                        target_mixer: None,
                    },
                });

                // カウントアップして同一フレーム内での重複を防ぐ
                bootstrap_in_flight.insert(mixer_entity);

                info!(
                    "AUTO_REFINE: Issued Refine task for MudMixer {:?}",
                    mixer_entity
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::relationships::{StoredIn, WorkingOn};
    use hw_jobs::{RefineData, RefinePhase, sync_refine_activity_index_system};
    use hw_logistics::{ResourceItem, Stockpile};

    fn spawn_ready_mixer(app: &mut App, x: f32) -> Entity {
        let mixer = app
            .world_mut()
            .spawn((
                Transform::from_xyz(x, 0.0, 0.0),
                MudMixerStorage {
                    sand: 1,
                    rock: 1,
                    mud: 0,
                },
                Stockpile {
                    capacity: 4,
                    resource_type: Some(ResourceType::Water),
                },
            ))
            .id();
        app.world_mut()
            .spawn((ResourceItem(ResourceType::Water), StoredIn(mixer)));
        mixer
    }

    #[test]
    fn auto_refine_skips_pending_mixer_and_keeps_live_control() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DesignationRequest>()
            .add_systems(Update, mud_mixer_auto_refine_system);
        app.world_mut().spawn(Yard {
            min: Vec2::splat(-100.0),
            max: Vec2::splat(100.0),
        });
        let pending = spawn_ready_mixer(&mut app, 0.0);
        let live = spawn_ready_mixer(&mut app, 20.0);
        let order = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(pending)
            .insert(DeconstructionPending { order });
        app.world_mut().flush();

        app.update();

        let requests = app
            .world_mut()
            .resource_mut::<Messages<DesignationRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].entity, live);
    }

    #[test]
    fn shared_activity_index_suppresses_all_refine_phases() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DesignationRequest>()
            .init_resource::<RefineActivityIndex>()
            .add_systems(
                Update,
                (
                    sync_refine_activity_index_system,
                    mud_mixer_auto_refine_system,
                )
                    .chain(),
            );
        app.world_mut().spawn(Yard {
            min: Vec2::splat(-100.0),
            max: Vec2::splat(100.0),
        });
        let mixer = spawn_ready_mixer(&mut app, 0.0);
        let soul = app
            .world_mut()
            .spawn(AssignedTask::Refine(RefineData {
                mixer,
                phase: RefinePhase::Done,
            }))
            .id();

        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Messages<DesignationRequest>>()
                .drain()
                .next()
                .is_none()
        );

        *app.world_mut()
            .entity_mut(soul)
            .get_mut::<AssignedTask>()
            .unwrap() = AssignedTask::None;
        app.update();
        let requests = app
            .world_mut()
            .resource_mut::<Messages<DesignationRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].entity, mixer);
    }

    #[test]
    fn task_workers_remain_an_independent_refine_guard() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DesignationRequest>()
            .init_resource::<RefineActivityIndex>()
            .add_systems(
                Update,
                (
                    sync_refine_activity_index_system,
                    mud_mixer_auto_refine_system,
                )
                    .chain(),
            );
        app.world_mut().spawn(Yard {
            min: Vec2::splat(-100.0),
            max: Vec2::splat(100.0),
        });
        let mixer = spawn_ready_mixer(&mut app, 0.0);
        let worker = app
            .world_mut()
            .spawn((AssignedTask::None, WorkingOn(mixer)))
            .id();
        app.world_mut().flush();

        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Messages<DesignationRequest>>()
                .drain()
                .next()
                .is_none()
        );

        app.world_mut().entity_mut(worker).remove::<WorkingOn>();
        app.world_mut().flush();
        app.update();
        let requests = app
            .world_mut()
            .resource_mut::<Messages<DesignationRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].entity, mixer);
    }
}
