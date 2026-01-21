//! 使い魔の分隊管理モジュール
//!
//! 分隊の構築・検証・解放ロジックを提供します。

use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path,
};
use crate::entities::familiar::UnderCommand;
use crate::relationships::{Commanding, Holding};
use crate::systems::jobs::{Designation, DesignationCreatedEvent};
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::work::unassign_task;
use crate::systems::visual::speech::components::{
    BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble,
};
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
                Option<&Holding>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
    ) -> (Vec<Entity>, Vec<Entity>) {
        let mut valid_squad = Vec::new();
        let mut invalid_members = Vec::new();

        for &member_entity in &squad {
            match q_souls.get(member_entity) {
                Ok((entity, _, _, _, _, _, _, _, uc, _)) => {
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
                            "FAM_AI: {:?} squad member {:?} has no UnderCommand comp yet (waiting sync)",
                            fam_entity, member_entity
                        );
                        // ここでは無効としない（次のフレームで再チェック）
                    }

                    valid_squad.push(entity);
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
        commands: &mut Commands,
        q_souls: &mut Query<
            (
                Entity,
                &Transform,
                &DamnedSoul,
                &mut AssignedTask,
                &mut Destination,
                &mut Path,
                &IdleState,
                Option<&Holding>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
        q_designations: &Query<
            (
                Entity,
                &Transform,
                &Designation,
                Option<&crate::systems::jobs::IssuedBy>,
                Option<&crate::systems::jobs::TaskSlots>,
                Option<&crate::relationships::TaskWorkers>,
            ),
        >,
        haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
        ev_created: &mut MessageWriter<DesignationCreatedEvent>,
        cooldowns: &mut crate::systems::visual::speech::cooldown::BubbleCooldowns,
        time: &Time,
        game_assets: &Res<crate::assets::GameAssets>,
        q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
        fam_transform: &Transform,
        voice_opt: Option<&crate::entities::familiar::FamiliarVoice>,
    ) -> Vec<Entity> {
        let mut released_entities = Vec::new();

        for &member_entity in squad {
            if let Ok((
                entity,
                transform,
                soul,
                mut task,
                _,
                mut path,
                idle,
                holding_opt,
                _,
                _participating_opt,
            )) = q_souls.get_mut(member_entity)
            {
                // 疲労・崩壊チェック
                let is_resting = idle.behavior == IdleBehavior::Gathering;
                if (!is_resting && soul.fatigue > fatigue_threshold)
                    || idle.behavior == IdleBehavior::ExhaustedGathering
                {
                    debug!(
                        "FAM_AI: {:?} releasing soul {:?} (Fatigue/Exhausted)",
                        fam_entity, member_entity
                    );

                    // タスクを解除
                    unassign_task(
                        commands,
                        entity,
                        transform.translation.truncate(),
                        &mut task,
                        &mut path,
                        holding_opt,
                        q_designations,
                        haul_cache,
                        Some(ev_created),
                        false, // emit_abandoned_event: 疲労リリース時は個別のタスク中断セリフを出さない
                    );

                    // リリースフレーズを表示
                    if cooldowns.can_speak(fam_entity, BubblePriority::Normal, time.elapsed_secs())
                    {
                        crate::systems::visual::speech::spawn::spawn_familiar_bubble(
                            commands,
                            fam_entity,
                            crate::systems::visual::speech::phrases::LatinPhrase::Abi,
                            fam_transform.translation,
                            game_assets,
                            q_bubbles,
                            BubbleEmotion::Neutral,
                            BubblePriority::Normal,
                            voice_opt,
                        );
                        cooldowns.record_speech(
                            fam_entity,
                            BubblePriority::Normal,
                            time.elapsed_secs(),
                        );
                    }

                    // UnderCommand を削除
                    commands.entity(member_entity).remove::<UnderCommand>();
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
