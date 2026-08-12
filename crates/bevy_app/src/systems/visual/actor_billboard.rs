use crate::entities::damned_soul::movement::animation::resolve_soul_billboard_frame;
use crate::entities::damned_soul::{
    AnimationState, ConversationExpression, DamnedSoul, IdleState, StressBreakdown,
};
use crate::plugins::startup::{Camera3dRtt, SoulBillboardHandles};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_visual::{ActorBillboard3d, SoulBillboardFrame, SoulProxyOwnerCache};

type BillboardOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static IdleState,
        &'static AssignedTask,
        &'static AnimationState,
        Option<&'static StressBreakdown>,
        Option<&'static ConversationExpression>,
    ),
    (With<DamnedSoul>, Without<ActorBillboard3d>),
>;

type ActorBillboardQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ActorBillboard3d,
        &'static mut SoulBillboardFrame,
        &'static mut Transform,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
    (Without<DamnedSoul>, Without<Camera3dRtt>),
>;

pub fn register_actor_billboard_system(
    added: Query<(Entity, &ActorBillboard3d), Added<ActorBillboard3d>>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for (entity, billboard) in &added {
        cache.actor_billboard.insert(billboard.owner, entity);
    }
}

pub fn cleanup_actor_billboard_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for owner in removed.read() {
        if let Some(entity) = cache.actor_billboard.remove(&owner) {
            commands.entity(entity).despawn();
        }
    }
}

pub fn sync_actor_billboard_system(
    owners: BillboardOwnerQuery,
    camera: Query<&Transform, With<Camera3dRtt>>,
    handles: Res<SoulBillboardHandles>,
    mut billboards: ActorBillboardQuery,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    for (billboard, mut applied_frame, mut visual_transform, mut material) in &mut billboards {
        let Ok((owner_transform, idle, task, animation, breakdown, expression)) =
            owners.get(billboard.owner)
        else {
            continue;
        };
        let working_or_moving = !matches!(*task, AssignedTask::None) || animation.is_moving;
        let frame = resolve_soul_billboard_frame(idle, breakdown, expression, working_or_moving);
        let facing_scale = if animation.facing_right { -1.0 } else { 1.0 };
        let next_transform = Transform {
            translation: Vec3::new(
                owner_transform.translation.x,
                TILE_SIZE * 0.55,
                -owner_transform.translation.y,
            ),
            rotation: camera.rotation,
            scale: Vec3::new(facing_scale, 1.0, 1.0),
        };
        if *applied_frame != frame {
            *applied_frame = frame;
        }
        let next_material = handles.material(frame);
        if material.0 != next_material {
            material.0 = next_material;
        }
        if *visual_transform != next_transform {
            *visual_transform = next_transform;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billboard_frame_pool_is_finite() {
        let frames = [
            SoulBillboardFrame::Normal,
            SoulBillboardFrame::Exhausted,
            SoulBillboardFrame::Happy,
            SoulBillboardFrame::Sleep,
            SoulBillboardFrame::Wine,
            SoulBillboardFrame::Trump,
            SoulBillboardFrame::Stress,
            SoulBillboardFrame::StressBreakdown,
        ];
        assert_eq!(frames.len(), 8);
    }
}
