use super::components::BubblePriority;
use bevy::prelude::*;

/// 吹き出しの履歴を管理するコンポーネント
#[derive(Component, Default, Debug, Reflect)]
#[reflect(Component)]
pub struct SpeechHistory {
    /// 最後に発言した時刻（秒）
    pub last_time: f32,
    /// その時の優先度
    pub last_priority: BubblePriority,
}

impl SpeechHistory {
    /// 発言可能かどうかを判定する
    /// - 高優先度 (High, Critical) は常に発言可能
    /// - 低優先度 は前回の発言から一定時間経過が必要
    pub fn can_speak(&self, priority: BubblePriority, current_time: f32) -> bool {
        // 高優先度はクールダウンを無視
        if priority >= BubblePriority::High {
            return true;
        }

        // 前回の発言がより高い優先度だった場合、低優先度は少し長めに待つ
        let cooldown = match self.last_priority {
            BubblePriority::Low => 0.5,
            BubblePriority::Normal => 1.0,
            _ => 1.5,
        };

        current_time - self.last_time > cooldown
    }

    /// 発言時刻を記録する
    pub fn record_speech(&mut self, priority: BubblePriority, current_time: f32) {
        self.last_time = current_time;
        self.last_priority = priority;
    }
}
