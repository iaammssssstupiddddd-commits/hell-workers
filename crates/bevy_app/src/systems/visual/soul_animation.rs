use crate::assets::GameAssets;
use crate::entities::damned_soul::{ConversationExpression, ConversationExpressionKind};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use hw_core::constants::EMOTION_THRESHOLD_EXHAUSTED;
use hw_core::soul::{AnimationState, DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};
use hw_visual::{
    CharacterMaterial, SoulAnimVisualState, SoulAnimationPlayer3d, SoulBodyAnimState,
    SoulFaceMaterial3d, SoulFaceState, soul_face_uv_offset,
};
use std::time::Duration;

const WALK_SIDE_ENTER_THRESHOLD: f32 = 0.9;
const WALK_SIDE_EXIT_THRESHOLD: f32 = 0.7;
const WALK_DELTA_EPSILON: f32 = 0.01;
const WALK_VARIANT_LOCK_SECS: f32 = 0.25;

#[derive(Resource, Default)]
pub struct SoulAnimationLibrary {
    pub graph: Option<Handle<AnimationGraph>>,
    pub idle: Option<AnimationNodeIndex>,
    pub walk: Option<AnimationNodeIndex>,
    pub work: Option<AnimationNodeIndex>,
    pub carry: Option<AnimationNodeIndex>,
    pub fear: Option<AnimationNodeIndex>,
    pub exhausted: Option<AnimationNodeIndex>,
    pub walk_left: Option<AnimationNodeIndex>,
    pub walk_right: Option<AnimationNodeIndex>,
    pub ready: bool,
}

impl SoulAnimationLibrary {
    fn node_for(
        &self,
        state: SoulBodyAnimState,
        walk_facing_right: Option<bool>,
    ) -> Option<AnimationNodeIndex> {
        match state {
            SoulBodyAnimState::Idle => self.idle,
            SoulBodyAnimState::Walk => match walk_facing_right {
                Some(true) => self.walk_left.or(self.walk).or(self.idle),
                Some(false) => self.walk_right.or(self.walk).or(self.idle),
                None => self.walk.or(self.idle),
            },
            SoulBodyAnimState::Work => self.work.or(self.idle),
            SoulBodyAnimState::Carry => match walk_facing_right {
                Some(true) => self.walk_left.or(self.carry).or(self.walk).or(self.idle),
                Some(false) => self.walk_right.or(self.carry).or(self.walk).or(self.idle),
                None => self.carry.or(self.walk).or(self.idle),
            },
            SoulBodyAnimState::Fear => self.fear.or(self.idle),
            SoulBodyAnimState::Exhausted => self.exhausted.or(self.idle),
        }
    }
}

type SoulAnimOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut SoulAnimVisualState,
        &'static DamnedSoul,
        &'static IdleState,
        &'static AnimationState,
        &'static SoulTaskVisualState,
        Option<&'static StressBreakdown>,
        Option<&'static ConversationExpression>,
    ),
>;
type SoulAnimationPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut SoulAnimationPlayer3d,
        &'static mut AnimationPlayer,
        Option<&'static AnimationGraphHandle>,
        Option<&'static mut AnimationTransitions>,
    ),
>;
type SoulAnimationOwnerStateQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SoulAnimVisualState,
        &'static AnimationState,
        &'static Transform,
    ),
    With<DamnedSoul>,
>;
type SoulFaceMaterialQuery<'w, 's> = Query<'w, 's, &'static SoulFaceMaterial3d>;

pub fn prepare_soul_animation_library_system(
    game_assets: Res<GameAssets>,
    gltfs: Res<Assets<Gltf>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    mut library: ResMut<SoulAnimationLibrary>,
) {
    if library.ready {
        return;
    }

    let Some(gltf) = gltfs.get(&game_assets.soul_gltf) else {
        return;
    };

    let Some(idle_clip) = gltf.named_animations.get("Idle").cloned() else {
        return;
    };
    let Some(walk_clip) = gltf.named_animations.get("Walk").cloned() else {
        return;
    };

    let mut graph = AnimationGraph::new();
    let idle = Some(graph.add_clip(idle_clip, 1.0, graph.root));
    let walk = Some(graph.add_clip(walk_clip, 1.0, graph.root));
    let work = gltf
        .named_animations
        .get("Work")
        .cloned()
        .map(|clip| graph.add_clip(clip, 1.0, graph.root));
    let carry = gltf
        .named_animations
        .get("Carry")
        .cloned()
        .map(|clip| graph.add_clip(clip, 1.0, graph.root));
    let fear = gltf
        .named_animations
        .get("Fear")
        .cloned()
        .map(|clip| graph.add_clip(clip, 1.0, graph.root));
    let exhausted = gltf
        .named_animations
        .get("Exhausted")
        .cloned()
        .map(|clip| graph.add_clip(clip, 1.0, graph.root));
    let walk_left = gltf
        .named_animations
        .get("WalkLeft")
        .cloned()
        .map(|clip| graph.add_clip(clip, 1.0, graph.root));
    let walk_right = gltf
        .named_animations
        .get("WalkRight")
        .cloned()
        .map(|clip| graph.add_clip(clip, 1.0, graph.root));

    *library = SoulAnimationLibrary {
        graph: Some(animation_graphs.add(graph)),
        idle,
        walk,
        work,
        carry,
        fear,
        exhausted,
        walk_left,
        walk_right,
        ready: true,
    };
}

pub fn initialize_soul_animation_players_system(
    mut commands: Commands,
    library: Res<SoulAnimationLibrary>,
    mut players: SoulAnimationPlayerQuery,
) {
    if !library.ready {
        return;
    }

    let Some(graph) = library.graph.clone() else {
        return;
    };
    let Some(idle_node) = library.node_for(SoulBodyAnimState::Idle, None) else {
        return;
    };

    for (entity, _binding, mut player, graph_handle_opt, transitions_opt) in &mut players {
        if graph_handle_opt.is_some() && transitions_opt.is_some() {
            continue;
        }

        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, idle_node, Duration::ZERO)
            .repeat();

        commands
            .entity(entity)
            .insert((AnimationGraphHandle(graph.clone()), transitions));
    }
}

pub fn sync_soul_anim_visual_state_system(mut q_souls: SoulAnimOwnerQuery) {
    for (mut anim_state, soul, idle, animation, task_visual, breakdown, expression) in &mut q_souls
    {
        let next_body =
            desired_body_state(soul, idle, animation, task_visual, breakdown, expression);
        let next_face =
            desired_face_state(soul, idle, animation, task_visual, breakdown, expression);

        if anim_state.body != next_body || anim_state.face != next_face {
            *anim_state = SoulAnimVisualState {
                body: next_body,
                face: next_face,
            };
        }
    }
}

pub fn sync_soul_body_animation_system(
    library: Res<SoulAnimationLibrary>,
    time: Res<Time>,
    q_states: SoulAnimationOwnerStateQuery,
    mut q_players: Query<(
        &mut SoulAnimationPlayer3d,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
) {
    if !library.ready {
        return;
    }

    for (mut binding, mut player, mut transitions) in &mut q_players {
        binding.directional_variant_lock_secs =
            (binding.directional_variant_lock_secs - time.delta_secs()).max(0.0);
        let Ok((state, animation, transform)) = q_states.get(binding.owner) else {
            continue;
        };
        let current_pos = transform.translation.truncate();
        let walk_facing_right = desired_walk_facing_right(
            state.body,
            animation,
            current_pos,
            binding.last_owner_pos,
            binding.walk_facing_right,
        );
        binding.last_owner_pos = Some(current_pos);
        if state.body == binding.current_body && walk_facing_right == binding.walk_facing_right {
            continue;
        }
        let directional_variant_changed = uses_directional_side_variant(state.body)
            && uses_directional_side_variant(binding.current_body)
            && walk_facing_right != binding.walk_facing_right;
        if directional_variant_changed && binding.directional_variant_lock_secs > 0.0 {
            continue;
        }
        let Some(node) = library.node_for(state.body, walk_facing_right) else {
            continue;
        };

        transitions
            .play(&mut player, node, Duration::from_millis(180))
            .repeat();
        binding.current_body = state.body;
        binding.walk_facing_right = walk_facing_right;
        binding.directional_variant_lock_secs = if uses_directional_side_variant(state.body) {
            WALK_VARIANT_LOCK_SECS
        } else {
            0.0
        };
    }
}

pub fn sync_soul_face_expression_system(
    q_states: Query<&SoulAnimVisualState, With<DamnedSoul>>,
    q_face_materials: SoulFaceMaterialQuery,
    mut materials: ResMut<Assets<CharacterMaterial>>,
) {
    for face in &q_face_materials {
        let Ok(state) = q_states.get(face.owner) else {
            continue;
        };
        let Some(material) = materials.get_mut(&face.material) else {
            continue;
        };
        material.set_face_uv_offset(face_uv_offset_for_state(state.face));
    }
}

fn desired_body_state(
    _soul: &DamnedSoul,
    idle: &IdleState,
    animation: &AnimationState,
    task_visual: &SoulTaskVisualState,
    breakdown: Option<&StressBreakdown>,
    _expression: Option<&ConversationExpression>,
) -> SoulBodyAnimState {
    if let Some(breakdown) = breakdown {
        return if breakdown.is_frozen {
            SoulBodyAnimState::Idle
        } else {
            SoulBodyAnimState::Fear
        };
    }

    if matches!(idle.behavior, IdleBehavior::ExhaustedGathering) {
        return SoulBodyAnimState::Exhausted;
    }

    if animation.is_moving {
        return if is_carry_phase(task_visual.phase) {
            SoulBodyAnimState::Carry
        } else {
            SoulBodyAnimState::Walk
        };
    }

    if is_carry_phase(task_visual.phase) {
        return SoulBodyAnimState::Carry;
    }

    if is_work_phase(task_visual.phase) {
        return SoulBodyAnimState::Work;
    }

    SoulBodyAnimState::Idle
}

fn desired_face_state(
    soul: &DamnedSoul,
    idle: &IdleState,
    animation: &AnimationState,
    task_visual: &SoulTaskVisualState,
    breakdown: Option<&StressBreakdown>,
    expression: Option<&ConversationExpression>,
) -> SoulFaceState {
    if breakdown.is_some() || matches_negative_expression(expression) {
        return SoulFaceState::Fear;
    }

    if matches_positive_expression(expression) {
        return SoulFaceState::Happy;
    }

    let is_busy = animation.is_moving || task_visual.phase != SoulTaskPhaseVisual::None;
    if matches!(
        idle.behavior,
        IdleBehavior::Sleeping | IdleBehavior::Resting
    ) && !is_busy
    {
        return SoulFaceState::Sleep;
    }

    if soul.fatigue >= EMOTION_THRESHOLD_EXHAUSTED
        || matches!(idle.behavior, IdleBehavior::ExhaustedGathering)
        || matches_exhausted_expression(expression)
    {
        return SoulFaceState::Exhausted;
    }

    if is_work_phase(task_visual.phase) {
        return SoulFaceState::Focused;
    }

    SoulFaceState::Normal
}

fn desired_walk_facing_right(
    body: SoulBodyAnimState,
    animation: &AnimationState,
    current_pos: Vec2,
    last_owner_pos: Option<Vec2>,
    current_walk_facing_right: Option<bool>,
) -> Option<bool> {
    if !animation.is_moving || !matches!(body, SoulBodyAnimState::Walk | SoulBodyAnimState::Carry) {
        return None;
    }

    let Some(last_pos) = last_owner_pos else {
        return current_walk_facing_right;
    };
    let movement = current_pos - last_pos;
    let movement_len_sq = movement.length_squared();
    if movement_len_sq <= WALK_DELTA_EPSILON * WALK_DELTA_EPSILON {
        return current_walk_facing_right;
    }

    let horizontal_ratio = movement.x.abs() / movement.length();
    let threshold = if current_walk_facing_right.is_some() {
        WALK_SIDE_EXIT_THRESHOLD
    } else {
        WALK_SIDE_ENTER_THRESHOLD
    };

    if horizontal_ratio >= threshold {
        Some(movement.x > 0.0)
    } else {
        None
    }
}

fn uses_directional_side_variant(body: SoulBodyAnimState) -> bool {
    matches!(body, SoulBodyAnimState::Walk | SoulBodyAnimState::Carry)
}

fn face_uv_offset_for_state(state: SoulFaceState) -> Vec2 {
    match state {
        SoulFaceState::Normal => soul_face_uv_offset(0.0, 0.0),
        SoulFaceState::Fear => soul_face_uv_offset(1.0, 0.0),
        SoulFaceState::Exhausted => soul_face_uv_offset(2.0, 0.0),
        SoulFaceState::Focused => soul_face_uv_offset(0.0, 1.0),
        SoulFaceState::Happy => soul_face_uv_offset(1.0, 1.0),
        SoulFaceState::Sleep => soul_face_uv_offset(2.0, 1.0),
    }
}

fn is_carry_phase(phase: SoulTaskPhaseVisual) -> bool {
    matches!(
        phase,
        SoulTaskPhaseVisual::Haul
            | SoulTaskPhaseVisual::HaulToBlueprint
            | SoulTaskPhaseVisual::BucketTransport
            | SoulTaskPhaseVisual::HaulToMixer
            | SoulTaskPhaseVisual::HaulWithWheelbarrow
    )
}

fn is_work_phase(phase: SoulTaskPhaseVisual) -> bool {
    matches!(
        phase,
        SoulTaskPhaseVisual::GatherChop
            | SoulTaskPhaseVisual::GatherMine
            | SoulTaskPhaseVisual::Build
            | SoulTaskPhaseVisual::ReinforceFloor
            | SoulTaskPhaseVisual::PourFloor
            | SoulTaskPhaseVisual::FrameWall
            | SoulTaskPhaseVisual::CoatWall
            | SoulTaskPhaseVisual::Refine
            | SoulTaskPhaseVisual::CollectBone
            | SoulTaskPhaseVisual::MovePlant
    )
}

fn matches_negative_expression(expression: Option<&ConversationExpression>) -> bool {
    matches!(
        expression.map(|expr| expr.kind),
        Some(ConversationExpressionKind::Negative)
    )
}

fn matches_exhausted_expression(expression: Option<&ConversationExpression>) -> bool {
    matches!(
        expression.map(|expr| expr.kind),
        Some(ConversationExpressionKind::Exhausted)
    )
}

fn matches_positive_expression(expression: Option<&ConversationExpression>) -> bool {
    matches!(
        expression.map(|expr| expr.kind),
        Some(
            ConversationExpressionKind::Positive
                | ConversationExpressionKind::GatheringWine
                | ConversationExpressionKind::GatheringTrump
        )
    )
}
