use crate::systems::jobs::WorkType;
use bevy::prelude::*;

/// 魂がタスクに割り当てられた
#[derive(Message, EntityEvent)]
pub struct OnTaskAssigned {
    pub entity: Entity, // Observerのターゲット（魂）
    pub task_entity: Entity,
    pub work_type: WorkType,
}

/// 魂がタスクを完了した
#[derive(Message, EntityEvent)]
pub struct OnTaskCompleted {
    pub entity: Entity, // 魂
    pub task_entity: Entity,
    pub work_type: WorkType,
}

/// 魂が使い魔に勧誘（使役開始）された
#[derive(Message, EntityEvent)]
pub struct OnSoulRecruited {
    pub entity: Entity, // 魂
    pub familiar_entity: Entity,
}

/// ストレスによるブレイクダウン/// ストレスが限界に達した
#[derive(Message, EntityEvent)]
pub struct OnStressBreakdown {
    pub entity: Entity,
}

/// 疲労が限界に達した（強制集会へ）
#[derive(Message, EntityEvent)]
pub struct OnExhausted {
    pub entity: Entity, // 魂
}

/// 魂が使い魔の使役から解放された
#[derive(Message, EntityEvent)]
pub struct OnReleasedFromService {
    pub entity: Entity,
}

/// 魂が自発的に集会に参加した
#[derive(Message, EntityEvent)]
pub struct OnGatheringJoined {
    pub entity: Entity,
}

/// 魂のタスクが中断・放棄された
#[derive(Message, EntityEvent)]
pub struct OnTaskAbandoned {
    pub entity: Entity,
}

/// 使い魔の使役数上限が変更された
#[derive(Message)]
pub struct FamiliarOperationMaxSoulChangedEvent {
    pub familiar_entity: Entity,
    pub old_value: usize,
    pub new_value: usize,
}

/// 魂が集会に参加した（スポット管理用）
#[derive(Message, EntityEvent)]
pub struct OnGatheringParticipated {
    pub entity: Entity,
    pub spot_entity: Entity,
}

/// 魂が集会から離脱した（スポット管理用）
#[derive(Message, EntityEvent)]
pub struct OnGatheringLeft {
    pub entity: Entity,
    pub spot_entity: Entity,
}

/// 使い魔が魂を激励した
#[derive(Message, EntityEvent)]
pub struct OnEncouraged {
    pub familiar_entity: Entity,
    #[event_target]
    pub soul_entity: Entity,
}

/// 使い魔のAI状態が変更された
#[derive(Message)]
pub struct FamiliarAiStateChangedEvent {
    /// 使い魔のエンティティ
    pub familiar_entity: Entity,
    /// 遷移前の状態
    pub from: crate::systems::familiar_ai::FamiliarAiState,
    /// 遷移後の状態
    pub to: crate::systems::familiar_ai::FamiliarAiState,
    /// 遷移の理由
    pub reason: FamiliarAiStateTransitionReason,
}

/// 状態遷移の理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliarAiStateTransitionReason {
    CommandChanged,
    SquadEmpty,
    SquadFull,
    RecruitSuccess,
    ScoutingCancelled,
    Unknown,
}
