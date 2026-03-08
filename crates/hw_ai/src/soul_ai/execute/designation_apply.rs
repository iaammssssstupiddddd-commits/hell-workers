use bevy::prelude::*;

use hw_core::events::{DesignationOp, DesignationRequest};
use hw_jobs::{Designation, IssuedBy, Priority, TargetBlueprint, TaskSlots};
use hw_jobs::mud_mixer::TargetMixer;
use hw_logistics::types::ReservedForTask;

/// Decide フェーズで生成された Designation 要求を適用する
pub fn apply_designation_requests_system(
    mut commands: Commands,
    mut request_reader: MessageReader<DesignationRequest>,
) {
    for request in request_reader.read() {
        match &request.operation {
            DesignationOp::Issue {
                work_type,
                issued_by,
                task_slots,
                priority,
                target_blueprint,
                target_mixer,
                reserved_for_task,
            } => {
                let mut entity = commands.entity(request.entity);
                entity.insert((
                    Designation {
                        work_type: *work_type,
                    },
                    IssuedBy(*issued_by),
                    TaskSlots::new(*task_slots),
                ));

                if let Some(p) = priority {
                    entity.insert(Priority(*p));
                }

                if let Some(target) = target_blueprint {
                    entity.insert(TargetBlueprint(*target));
                }

                if let Some(target) = target_mixer {
                    entity.insert(TargetMixer(*target));
                }

                if *reserved_for_task {
                    entity.insert(ReservedForTask);
                }
            }
        }
    }
}
