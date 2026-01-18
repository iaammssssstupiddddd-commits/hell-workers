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
