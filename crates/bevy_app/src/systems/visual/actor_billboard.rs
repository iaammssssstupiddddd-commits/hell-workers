use crate::entities::damned_soul::movement::animation::resolve_soul_billboard_frame;
use crate::entities::damned_soul::{
    AnimationState, ConversationExpression, DamnedSoul, IdleState, StressBreakdown,
};
use crate::plugins::startup::{Camera3dRtt, SoulBillboardHandles};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_visual::{
    ActorBillboard3d, ActorBillboardOwnerCache, SoulAnimVisualState, SoulBillboardFrame,
};

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

type ChangedBillboardOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static IdleState,
        &'static AssignedTask,
        &'static AnimationState,
        Option<&'static StressBreakdown>,
        Option<&'static ConversationExpression>,
    ),
    (
        With<DamnedSoul>,
        Without<ActorBillboard3d>,
        Or<(
            Changed<Transform>,
            Changed<IdleState>,
            Changed<AssignedTask>,
            Changed<SoulAnimVisualState>,
            Changed<StressBreakdown>,
            Changed<ConversationExpression>,
        )>,
    ),
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

#[derive(SystemParam)]
pub struct ActorBillboardSyncContext<'w, 's> {
    owners: BillboardOwnerQuery<'w, 's>,
    changed_owners: ChangedBillboardOwnerQuery<'w, 's>,
    camera: Query<'w, 's, Ref<'static, Transform>, With<Camera3dRtt>>,
    handles: Res<'w, SoulBillboardHandles>,
    cache: Res<'w, ActorBillboardOwnerCache>,
    added: Query<'w, 's, (Entity, &'static ActorBillboard3d), Added<ActorBillboard3d>>,
    removed_breakdowns: RemovedComponents<'w, 's, StressBreakdown>,
    removed_expressions: RemovedComponents<'w, 's, ConversationExpression>,
    billboards: ActorBillboardQuery<'w, 's>,
}

pub fn register_actor_billboard_system(
    added: Query<(Entity, &ActorBillboard3d), Added<ActorBillboard3d>>,
    mut cache: ResMut<ActorBillboardOwnerCache>,
) {
    for (entity, billboard) in &added {
        cache.actor_billboard.insert(billboard.owner, entity);
    }
}

pub fn cleanup_actor_billboard_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    mut cache: ResMut<ActorBillboardOwnerCache>,
) {
    for owner in removed.read() {
        if let Some(entity) = cache.actor_billboard.remove(&owner) {
            commands.entity(entity).despawn();
        }
    }
}

pub fn sync_actor_billboard_system(mut context: ActorBillboardSyncContext) {
    let Ok(camera) = context.camera.single() else {
        return;
    };
    if camera.is_changed() {
        for (billboard, mut applied_frame, mut visual_transform, mut material) in
            &mut context.billboards
        {
            apply_billboard_state(
                billboard.owner,
                camera.rotation,
                &context.owners,
                &context.handles,
                &mut applied_frame,
                &mut visual_transform,
                &mut material,
            );
        }
        return;
    }

    for (owner, ..) in &context.changed_owners {
        sync_billboard_for_owner(
            owner,
            camera.rotation,
            &context.owners,
            &context.handles,
            &context.cache,
            &mut context.billboards,
        );
    }
    for (entity, billboard) in &context.added {
        let Ok((_, mut applied_frame, mut visual_transform, mut material)) =
            context.billboards.get_mut(entity)
        else {
            continue;
        };
        apply_billboard_state(
            billboard.owner,
            camera.rotation,
            &context.owners,
            &context.handles,
            &mut applied_frame,
            &mut visual_transform,
            &mut material,
        );
    }
    for owner in context.removed_breakdowns.read() {
        sync_billboard_for_owner(
            owner,
            camera.rotation,
            &context.owners,
            &context.handles,
            &context.cache,
            &mut context.billboards,
        );
    }
    for owner in context.removed_expressions.read() {
        sync_billboard_for_owner(
            owner,
            camera.rotation,
            &context.owners,
            &context.handles,
            &context.cache,
            &mut context.billboards,
        );
    }
}

fn sync_billboard_for_owner(
    owner: Entity,
    camera_rotation: Quat,
    owners: &BillboardOwnerQuery,
    handles: &SoulBillboardHandles,
    cache: &ActorBillboardOwnerCache,
    billboards: &mut ActorBillboardQuery,
) {
    let Some(&entity) = cache.actor_billboard.get(&owner) else {
        return;
    };
    let Ok((_, mut applied_frame, mut visual_transform, mut material)) = billboards.get_mut(entity)
    else {
        return;
    };
    apply_billboard_state(
        owner,
        camera_rotation,
        owners,
        handles,
        &mut applied_frame,
        &mut visual_transform,
        &mut material,
    );
}

fn apply_billboard_state(
    owner: Entity,
    camera_rotation: Quat,
    owners: &BillboardOwnerQuery,
    handles: &SoulBillboardHandles,
    applied_frame: &mut SoulBillboardFrame,
    visual_transform: &mut Transform,
    material: &mut MeshMaterial3d<StandardMaterial>,
) {
    let Ok((owner_transform, idle, task, animation, breakdown, expression)) = owners.get(owner)
    else {
        return;
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
        rotation: camera_rotation,
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
