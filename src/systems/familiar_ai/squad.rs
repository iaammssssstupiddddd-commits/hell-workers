//! 使い魔の分隊管理モジュール
//!
//! 分隊の構築・検証・解放ロジックを提供します。

use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path,
};
use crate::relationships::CommandedBy;
use crate::relationships::Commanding;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
// use crate::systems::soul_ai::work::unassign_task;
// use crate::systems::visual::speech::components::{
//     BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble,
// };
use bevy::prelude::*;

/// 分隊管理ユーティリティ
pub struct SquadManager;

impl SquadManager {
    /// Relationship から分隊を構築
    pub fn build_squad(commanding: Option<&Commanding>) -> Vec<Entity> {
        if let Some(c) = commanding {
            c.iter().copied().collect()
        } else {
            Vec::new()
        }
    }

    /// 分隊を検証し、無効なメンバーを除外
    ///
    /// # 引数
    /// - `squad`: 検証する分隊メンバーのリスト
    /// - `fam_entity`: 使い魔のエンティティ
    /// - `q_souls`: 魂のクエリ
    ///
    /// # 戻り値
    /// 有効なメンバーのリストと、無効なメンバーのリスト
    pub fn validate_squad(
        squad: Vec<Entity>,
        fam_entity: Entity,
        q_souls: &Query<
            (
                Entity,
                &Transform,
                &DamnedSoul,
                &mut AssignedTask,
                &mut Destination,
                &mut Path,
                &IdleState,

                Option<&mut crate::systems::logistics::Inventory>,
                Option<&CommandedBy>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
    ) -> (Vec<Entity>, Vec<Entity>) {
        let mut valid_squad = Vec::new();
        let mut invalid_members = Vec::new();

        for &member_entity in &squad {
            match q_souls.get(member_entity) {
                Ok((_entity, _, _, _, _, _, _, _, uc, _)) => {
                    // 整合性チェック: 相手が自分を主人だと思っていないなら無効
                    if let Some(uc_comp) = uc {
                        if uc_comp.0 != fam_entity {
                            info!(
                                "FAM_AI: {:?} squad member {:?} belongs to another master {:?}",
                                fam_entity, member_entity, uc_comp.0
                            );
                            invalid_members.push(member_entity);
                            continue;
                        }
                    } else {
                        // Relationship の反映がコンポーネントより先に来る可能性があるため
                        // 1フレーム待つ（警告のみ）
                        debug!(
                            "FAM_AI: {:?} squad member {:?} has no CommandedBy comp yet (waiting sync)",
                            fam_entity, member_entity
                        );
                        // ここでは無効としない（次のフレームで再チェック）
                    }

                    valid_squad.push(member_entity);
                }
                Err(_) => {
                    // エンティティが消失している
                    invalid_members.push(member_entity);
                }
            }
        }

        (valid_squad, invalid_members)
    }

    /// 疲労・崩壊したメンバーをリリース
    ///
    /// # 引数
    /// - `squad`: 分隊メンバーのリスト
    /// - `fam_entity`: 使い魔のエンティティ
    /// - `fatigue_threshold`: 疲労閾値
    /// - `commands`: Commands
    /// - `q_souls`: 魂のクエリ
    /// - `q_designations`: タスクのクエリ
    /// - `haul_cache`: 搬送キャッシュ
    /// - `ev_created`: タスク作成イベント
    /// - `cooldowns`: スピーチクールダウン
    /// - `time`: Time
    /// - `game_assets`: GameAssets
    /// - `q_bubbles`: 吹き出しクエリ
    /// - `fam_transform`: 使い魔のTransform
    /// - `voice_opt`: 声の設定（オプション）
    ///
    /// # 戻り値
    /// リリースされたメンバーのリスト
    pub fn release_fatigued(
        squad: &[Entity],
        fam_entity: Entity,
        fatigue_threshold: f32,
    q_souls: &Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            Option<&mut crate::systems::logistics::Inventory>,
            Option<&CommandedBy>,
            Option<&ParticipatingIn>,
        ),
        Without<crate::entities::familiar::Familiar>,
    >,
        request_writer: &mut MessageWriter<crate::events::SquadManagementRequest>,
    ) -> Vec<Entity> {
        let mut released_entities = Vec::new();

        for &member_entity in squad {
            if let Ok((
                _entity,
                _transform,
                soul,
                _task,
                _dest,
                _path,
                idle,
                _inv,
                _cb,
                _pi,
            )) = q_souls.get(member_entity)
            {
                // 疲労・崩壊チェック
                let is_resting = idle.behavior == IdleBehavior::Gathering;
                if (!is_resting && soul.fatigue > fatigue_threshold)
                    || idle.behavior == IdleBehavior::ExhaustedGathering
                {
                    debug!(
                        "FAM_AI: {:?} requesting release of soul {:?} (Fatigue/Exhausted)",
                        fam_entity, member_entity
                    );

                    request_writer.write(crate::events::SquadManagementRequest {
                        familiar_entity: fam_entity,
                        operation: crate::events::SquadManagementOperation::ReleaseMember {
                            soul_entity: member_entity,
                            reason: crate::events::ReleaseReason::Fatigued,
                        },
                    });

                    released_entities.push(member_entity);
                }
            } else {
                // エンティティが消失している
                released_entities.push(member_entity);
            }
        }

        released_entities
    }
}
