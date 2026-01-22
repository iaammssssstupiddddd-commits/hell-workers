use super::components::BubblePriority;
use bevy::prelude::*;
use std::collections::HashMap;

/// 吹き出しのクールダウンを管理するリソース
#[derive(Resource, Default)]
pub struct BubbleCooldowns {
    /// Entity -> (最後に発言した時刻（秒）, その時の優先度)
    pub last_speech: HashMap<Entity, (f32, BubblePriority)>,
}

impl BubbleCooldowns {
    /// 発言可能かどうかを判定する
    /// - 高優先度 (High, Critical) は常に発言可能
    /// - 低優先度 は前回の発言から一定時間経過が必要
    pub fn can_speak(&self, entity: Entity, priority: BubblePriority, current_time: f32) -> bool {
        // 高優先度はクールダウンを無視
        if priority >= BubblePriority::High {
            return true;
        }

        if let Some((last_time, last_priority)) = self.last_speech.get(&entity) {
            // 前回の発言がより高い優先度だった場合、低優先度は少し長めに待つ
            let cooldown = match last_priority {
                BubblePriority::Low => 0.5,
                BubblePriority::Normal => 1.0,
                _ => 1.5,
            };

            current_time - last_time > cooldown
        } else {
            true
        }
    }

    /// 発言時刻を記録する
    pub fn record_speech(&mut self, entity: Entity, priority: BubblePriority, current_time: f32) {
        self.last_speech.insert(entity, (current_time, priority));
    }
}
