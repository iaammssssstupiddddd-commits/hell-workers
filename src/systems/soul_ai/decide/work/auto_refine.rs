//! Auto-refine system for MudMixer
//!
//! Automatically creates refine tasks when materials are ready in MudMixer.

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::events::{DesignationOp, DesignationRequest};
use crate::relationships::StoredItems;
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, MudMixerStorage, WorkType};
use crate::systems::logistics::{ResourceType, Stockpile};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;

/// MudMixer で精製タスクを自動発行するシステム
pub fn mud_mixer_auto_refine_system(
    mut designation_writer: MessageWriter<DesignationRequest>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_mixers: Query<(
        Entity,
        &Transform,
        &MudMixerStorage,
        Option<&TaskWorkers>,
        Option<&Designation>,
        Option<&Stockpile>,
        Option<&StoredItems>,
    )>,
    q_souls: Query<&AssignedTask>,
) {
    // 1. 集計フェーズ: 現在実行中の精製タスクをカウント
    let mut in_flight = std::collections::HashMap::<Entity, usize>::new();

    for task in q_souls.iter() {
        if let AssignedTask::Refine(data) = task {
            *in_flight.entry(data.mixer).or_insert(0) += 1;
        }
    }

    for (fam_entity, active_command, task_area) in q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        for (
            mixer_entity,
            mixer_transform,
            storage,
            workers_opt,
            designation_opt,
            stockpile_opt,
            stored_opt,
        ) in q_mixers.iter()
        {
            let mixer_pos = mixer_transform.translation.truncate();
            if !task_area.contains(mixer_pos) {
                continue;
            }

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
                            work_type: WorkType::Refine,
                            issued_by: fam_entity,
                            task_slots: 1,
                            priority: None,
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
}
