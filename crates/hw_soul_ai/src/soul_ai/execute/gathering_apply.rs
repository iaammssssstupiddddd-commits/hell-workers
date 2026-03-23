use bevy::prelude::*;

use hw_core::events::{GatheringManagementOp, GatheringManagementRequest, OnGatheringParticipated};
use hw_core::relationships::ParticipatingIn;

/// GatheringManagementRequest を適用する（Execute Phase）
pub fn gathering_apply_system(
    mut commands: Commands,
    mut request_reader: MessageReader<GatheringManagementRequest>,
    q_participating: Query<Option<&ParticipatingIn>>,
) {
    for request in request_reader.read() {
        match &request.operation {
            GatheringManagementOp::Dissolve {
                spot_entity,
                aura_entity,
                object_entity,
            } => {
                commands.entity(*aura_entity).despawn();
                if let Some(obj) = object_entity {
                    commands.entity(*obj).despawn();
                }
                commands.entity(*spot_entity).despawn();
            }
            GatheringManagementOp::Merge {
                absorber,
                absorbed,
                participants_to_move,
                absorbed_aura,
                absorbed_object,
            } => {
                for soul_entity in participants_to_move {
                    commands
                        .entity(*soul_entity)
                        .insert(ParticipatingIn(*absorber));
                    commands.trigger(OnGatheringParticipated {
                        entity: *soul_entity,
                        spot_entity: *absorber,
                    });
                }

                commands.entity(*absorbed_aura).despawn();
                if let Some(obj) = absorbed_object {
                    commands.entity(*obj).despawn();
                }
                commands.entity(*absorbed).despawn();
            }
            GatheringManagementOp::Recruit { soul, spot } => {
                let already_participating = q_participating
                    .get(*soul)
                    .ok()
                    .flatten()
                    .map(|p| p.0 == *spot)
                    .unwrap_or(false);

                if already_participating {
                    continue;
                }

                commands.entity(*soul).insert(ParticipatingIn(*spot));
                commands.trigger(OnGatheringParticipated {
                    entity: *soul,
                    spot_entity: *spot,
                });
            }
            GatheringManagementOp::Leave { soul, spot: _ } => {
                commands.entity(*soul).remove::<ParticipatingIn>();
            }
        }
    }
}
