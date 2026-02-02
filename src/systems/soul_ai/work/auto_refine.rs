//! Auto-refine system for MudMixer
//!
//! Automatically creates refine tasks when materials are ready in MudMixer.

use bevy::prelude::*;

use crate::entities::familiar::ActiveCommand;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, MudMixerStorage, TaskSlots, WorkType};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::relationships::TaskWorkers;

/// MudMixer で精製タスクを自動発行するシステム
pub fn mud_mixer_auto_refine_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_mixers: Query<(Entity, &Transform, &MudMixerStorage, Option<&TaskWorkers>, Option<&Designation>)>,
    q_souls: Query<&AssignedTask>,
) {
    // 1. 集計フェーズ: 現在実行中の精製タスクをカウント
    let mut in_flight = std::collections::HashMap::<Entity, usize>::new();

    for task in q_souls.iter() {
        if let AssignedTask::Refine(data) = task {
            *in_flight.entry(data.mixer).or_insert(0) += 1;
        }
    }

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        for (mixer_entity, mixer_transform, storage, workers_opt, designation_opt) in q_mixers.iter() {
            let mixer_pos = mixer_transform.translation.truncate();
            if !task_area.contains(mixer_pos) {
                continue;
            }

            // 既に Designation がある場合はスキップ
            if designation_opt.is_some() {
                continue;
            }

            // 原料が揃っているかチェック (1 Sand + 1 Water + 1 Rock)
            if storage.sand >= 1 && storage.water >= 1 && storage.rock >= 1 {
                let inflight_count = *in_flight.get(&mixer_entity).unwrap_or(&0);
                let current_workers = workers_opt.map(|w| w.len()).unwrap_or(0);

                // 作業員が1名未満（精製は1人で行う）かつ、予約中のタスクがない場合
                if current_workers + inflight_count < 1 {
                    // Refine タスクを発行
                    commands.entity(mixer_entity).insert((
                        Designation { work_type: WorkType::Refine },
                        IssuedBy(fam_entity),
                        TaskSlots::new(1),
                    ));

                    // カウントアップして同一フレーム内での重複を防ぐ
                    in_flight.insert(mixer_entity, inflight_count + 1);

                    info!("AUTO_REFINE: Issued Refine task for MudMixer {:?}", mixer_entity);
                }
            }
        }
    }
}
