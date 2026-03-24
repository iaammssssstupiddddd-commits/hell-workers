//! Auto-refine system for MudMixer
//!
//! Automatically creates refine tasks when materials are ready in MudMixer.

use bevy::prelude::*;

use hw_core::area::TaskArea;
use hw_core::constants::MUD_MIXER_REFINE_PRIORITY;
use hw_core::events::{DesignationOp, DesignationRequest};
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::logistics::ResourceType;
use hw_core::relationships::{StoredItems, TaskWorkers};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{AssignedTask, Designation, MovePlanned};
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
    ),
>;

/// MudMixer で精製タスクを自動発行するシステム
pub fn mud_mixer_auto_refine_system(
    mut designation_writer: MessageWriter<DesignationRequest>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_mixers: MixersQuery,
    q_souls: Query<&AssignedTask>,
) {
    // 1. 集計フェーズ: 現在実行中の精製タスクをカウント
    let mut in_flight = std::collections::HashMap::<Entity, usize>::new();

    for task in q_souls.iter() {
        if let AssignedTask::Refine(data) = task {
            *in_flight.entry(data.mixer).or_insert(0) += 1;
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
    ) in q_mixers.iter()
    {
        if move_planned_opt.is_some() {
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
            let inflight_count = *in_flight.get(&mixer_entity).unwrap_or(&0);
            let current_workers = workers_opt.map(|w| w.len()).unwrap_or(0);

            // 作業員が1名未満（精製は1人で行う）かつ、予約中のタスクがない場合
            if current_workers + inflight_count < 1 {
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
                        reserved_for_task: false,
                    },
                });

                // カウントアップして同一フレーム内での重複を防ぐ
                in_flight.insert(mixer_entity, inflight_count + 1);

                info!(
                    "AUTO_REFINE: Issued Refine task for MudMixer {:?}",
                    mixer_entity
                );
            }
        }
    }
}
