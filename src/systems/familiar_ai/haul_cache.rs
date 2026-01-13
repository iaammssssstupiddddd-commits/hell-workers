use bevy::prelude::*;
use std::collections::HashMap;

/// 搬送中のアイテム・ストックパイル予約状況をキャッシュするリソース
///
/// 毎フレーム全ソウルをイテレートして O(S) で計算していたのを、
/// イベント駆動（タスク割当・完了・中断時）で更新するようにし O(1) に改善。
#[derive(Resource, Default, Debug)]
pub struct HaulReservationCache {
    /// ストックパイルエンティティ -> 搬送中の予約数
    reservations: HashMap<Entity, usize>,
}

impl HaulReservationCache {
    /// 予約を追加
    pub fn reserve(&mut self, stockpile: Entity) {
        *self.reservations.entry(stockpile).or_insert(0) += 1;
        debug!(
            "HAUL_CACHE: Reserved stockpile {:?} (count: {})",
            stockpile, self.reservations[&stockpile]
        );
    }

    /// 予約を解除
    pub fn release(&mut self, stockpile: Entity) {
        if let Some(count) = self.reservations.get_mut(&stockpile) {
            *count = count.saturating_sub(1);
            let new_count = *count;
            if new_count == 0 {
                self.reservations.remove(&stockpile);
            }
            debug!(
                "HAUL_CACHE: Released stockpile {:?} (count: {})",
                stockpile, new_count
            );
        }
    }

    /// 特定のストックパイルの予約数を取得
    pub fn get(&self, stockpile: Entity) -> usize {
        self.reservations.get(&stockpile).cloned().unwrap_or(0)
    }

    /// 全てのキャッシュをクリア（初期化用）
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.reservations.clear();
    }
}
