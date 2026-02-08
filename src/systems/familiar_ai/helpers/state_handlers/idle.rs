//! Idle 状態のハンドラー
//!
//! プレイヤーからの Idle コマンドが発行された際の処理を行います。

use super::StateTransitionResult;
use crate::entities::damned_soul::{Destination, Path};
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::visual::speech::components::{
    BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble,
};
use bevy::prelude::*;

/// Idle 状態のハンドラー
///
/// # 引数
/// - `fam_entity`: 使い魔のエンティティ
/// - `fam_transform`: 使い魔のTransform
/// - `active_command`: アクティブなコマンド
/// - `ai_state`: AI状態（変更可能）
/// - `fam_dest`: 目的地（変更可能）
/// - `fam_path`: パス（変更可能）
/// - `commands`: Commands
/// - `time`: Time
/// - `game_assets`: GameAssets
/// - `q_bubbles`: 吹き出しクエリ
/// - `cooldowns`: クールダウン管理
/// - `voice_opt`: 声の設定（オプション）
pub fn handle_idle_state(
    fam_entity: Entity,
    fam_transform: &Transform,
    active_command: &ActiveCommand,
    ai_state: &mut FamiliarAiState,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
    commands: &mut Commands,
    time: &Res<Time>,
    game_assets: &Res<crate::assets::GameAssets>,
    q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    history_opt: Option<&mut crate::systems::visual::speech::cooldown::SpeechHistory>,
    voice_opt: Option<&crate::entities::familiar::FamiliarVoice>,
) -> StateTransitionResult {
    // Idle コマンドでない場合は処理しない
    if !matches!(active_command.command, FamiliarCommand::Idle) {
        return StateTransitionResult::Stay;
    }

    // 状態が Idle でない場合は遷移
    if *ai_state != FamiliarAiState::Idle {
        debug!(
            "FAM_AI: {:?} Switching to Idle state because command is Idle",
            fam_entity
        );
        *ai_state = FamiliarAiState::Idle;

        // 休息フレーズを表示
        let current_time = time.elapsed_secs();
        let can_speak = if let Some(history) = &history_opt {
            history.can_speak(BubblePriority::Normal, current_time)
        } else {
            true
        };

        if can_speak {
            crate::systems::visual::speech::spawn::spawn_familiar_bubble(
                commands,
                fam_entity,
                crate::systems::visual::speech::phrases::LatinPhrase::Requiesce,
                fam_transform.translation,
                game_assets,
                q_bubbles,
                BubbleEmotion::Neutral,
                BubblePriority::Normal,
                voice_opt,
            );
            if let Some(history) = history_opt {
                history.record_speech(BubblePriority::Normal, current_time);
            } else {
                commands.entity(fam_entity).insert(
                    crate::systems::visual::speech::cooldown::SpeechHistory {
                        last_time: current_time,
                        last_priority: BubblePriority::Normal,
                    },
                );
            }
        }
    }

    // 移動を停止
    fam_dest.0 = fam_transform.translation.truncate();
    fam_path.waypoints.clear();

    StateTransitionResult::Stay
}
