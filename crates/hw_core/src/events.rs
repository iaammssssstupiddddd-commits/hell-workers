use bevy::prelude::*;

/// 魂が使い魔に勧誘（使役開始）された
#[derive(Message, EntityEvent)]
pub struct OnSoulRecruited {
    pub entity: Entity,
    pub familiar_entity: Entity,
}

/// ストレスが限界に達した
#[derive(Message, EntityEvent)]
pub struct OnStressBreakdown {
    pub entity: Entity,
}

/// 疲労が限界に達した（強制集会へ）
#[derive(Message, EntityEvent)]
pub struct OnExhausted {
    pub entity: Entity,
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
#[derive(Event, Debug, Reflect)]
pub struct OnGatheringLeft {
    pub entity: Entity,
}

/// 使い魔が魂を激励した
#[derive(Message, EntityEvent)]
pub struct OnEncouraged {
    pub familiar_entity: Entity,
    #[event_target]
    pub soul_entity: Entity,
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

/// アイドル行動の変更要求
#[derive(Message, Debug, Clone)]
pub struct IdleBehaviorRequest {
    pub entity: Entity,
    pub operation: IdleBehaviorOperation,
}

/// アイドル行動の操作種別
#[derive(Debug, Clone)]
pub enum IdleBehaviorOperation {
    JoinGathering { spot_entity: Entity },
    LeaveGathering { spot_entity: Entity },
    ArriveAtGathering { spot_entity: Entity },
    ReserveRestArea { rest_area_entity: Entity },
    ReleaseRestArea,
    EnterRestArea { rest_area_entity: Entity },
    LeaveRestArea,
}

/// 逃走行動の変更要求
#[derive(Message, Debug, Clone)]
pub struct EscapeRequest {
    pub entity: Entity,
    pub operation: EscapeOperation,
}

/// 逃走行動の操作種別
#[derive(Debug, Clone)]
pub enum EscapeOperation {
    StartEscaping { leave_gathering: Option<Entity> },
    UpdateDestination { destination: Vec2 },
    ReachSafety,
    JoinSafeGathering,
}

/// 集会管理の変更要求
#[derive(Message, Debug, Clone)]
pub struct GatheringManagementRequest {
    pub operation: GatheringManagementOp,
}

/// 集会管理の操作種別
#[derive(Debug, Clone)]
pub enum GatheringManagementOp {
    Dissolve {
        spot_entity: Entity,
        aura_entity: Entity,
        object_entity: Option<Entity>,
    },
    Merge {
        absorber: Entity,
        absorbed: Entity,
        participants_to_move: Vec<Entity>,
        absorbed_aura: Entity,
        absorbed_object: Option<Entity>,
    },
    Recruit {
        soul: Entity,
        spot: Entity,
    },
    Leave {
        soul: Entity,
        spot: Entity,
    },
}

/// 使い魔による激励要求
#[derive(Message, Debug, Clone)]
pub struct EncouragementRequest {
    pub familiar_entity: Entity,
    pub soul_entity: Entity,
}

/// 使い魔のIdle遷移時ビジュアル要求
#[derive(Message, Debug, Clone)]
pub struct FamiliarIdleVisualRequest {
    pub familiar_entity: Entity,
}

/// 使い魔の分隊管理要求
#[derive(Message, Debug, Clone)]
pub struct SquadManagementRequest {
    pub familiar_entity: Entity,
    pub operation: SquadManagementOperation,
}

/// 分隊管理の操作種別
#[derive(Debug, Clone)]
pub enum SquadManagementOperation {
    AddMember { soul_entity: Entity },
    ReleaseMember {
        soul_entity: Entity,
        reason: ReleaseReason,
    },
}

/// 分隊解放の理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseReason {
    Fatigued,
}
