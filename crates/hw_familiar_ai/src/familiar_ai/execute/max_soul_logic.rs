//! 使役数上限変更時のロジック処理：超過分の魂のタスク解除とリリース。

use bevy::prelude::*;
use hw_core::events::{FamiliarOperationMaxSoulChangedEvent, SoulTaskUnassignRequest};
use hw_core::familiar::Familiar;
use hw_core::relationships::{CommandedBy, Commanding};

/// 使役数上限変更イベントで超過分の魂をリリースするロジックシステム
pub fn max_soul_logic_system(
    mut ev_max_soul_changed: MessageReader<FamiliarOperationMaxSoulChangedEvent>,
    q_commanding: Query<&Commanding, With<Familiar>>,
    mut task_unassign_writer: MessageWriter<SoulTaskUnassignRequest>,
    mut commands: Commands,
) {
    for event in ev_max_soul_changed.read() {
        if event.new_value >= event.old_value {
            continue;
        }
        let Ok(commanding) = q_commanding.get(event.familiar_entity) else {
            continue;
        };

        let squad_entities: Vec<Entity> = commanding.iter().copied().collect();
        if squad_entities.len() <= event.new_value {
            continue;
        }

        let excess_count = squad_entities.len() - event.new_value;
        info!(
            "FAM_AI: {:?} max_soul decreased from {} to {}, releasing {} excess members",
            event.familiar_entity, event.old_value, event.new_value, excess_count
        );

        let mut released_count = 0;
        for i in (0..squad_entities.len()).rev() {
            if released_count >= excess_count {
                break;
            }
            let member_entity = squad_entities[i];

            task_unassign_writer.write(SoulTaskUnassignRequest {
                soul_entity: member_entity,
                emit_abandoned: false,
            });

            commands.entity(member_entity).remove::<CommandedBy>();
            released_count += 1;

            info!(
                "FAM_AI: {:?} released excess member {:?} (limit: {} -> {})",
                event.familiar_entity, member_entity, event.old_value, event.new_value
            );
        }
    }
}

