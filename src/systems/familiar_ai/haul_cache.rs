use bevy::prelude::*;
use std::collections::HashMap;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::task_execution::AssignedTask;

/// 搬送中のアイテム・ストックパイル予約状況をキャッシュするリソース
///
/// 毎フレーム全ソウルをイテレートして O(S) で計算していたのを、
/// イベント駆動（タスク割当・完了・中断時）で更新するようにし O(1) に改善。
#[derive(Resource, Default, Debug)]
pub struct HaulReservationCache {
    /// ストックパイルエンティティ -> 搬送中の予約数
    reservations: HashMap<Entity, usize>,
    /// ミキサーエンティティ + リソースタイプ -> 搬送中の予約数
    mixer_reservations: HashMap<(Entity, ResourceType), usize>,
}

impl HaulReservationCache {
    /// 外部から予約状況を再構築する（タスク状態から同期）
    pub fn reset(
        &mut self,
        reservations: HashMap<Entity, usize>,
        mixer_reservations: HashMap<(Entity, ResourceType), usize>,
    ) {
        self.reservations = reservations;
        self.mixer_reservations = mixer_reservations;
    }
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

    /// ミキサー予約を追加
    pub fn reserve_mixer(&mut self, mixer: Entity, resource_type: ResourceType) {
        let key = (mixer, resource_type);
        *self.mixer_reservations.entry(key).or_insert(0) += 1;
        debug!(
            "HAUL_CACHE: Reserved mixer {:?} for {:?} (count: {})",
            mixer, resource_type, self.mixer_reservations[&key]
        );
    }

    /// ミキサー予約を解除
    pub fn release_mixer(&mut self, mixer: Entity, resource_type: ResourceType) {
        let key = (mixer, resource_type);
        if let Some(count) = self.mixer_reservations.get_mut(&key) {
            *count = count.saturating_sub(1);
            let new_count = *count;
            if new_count == 0 {
                self.mixer_reservations.remove(&key);
            }
            debug!(
                "HAUL_CACHE: Released mixer {:?} for {:?} (count: {})",
                mixer, resource_type, new_count
            );
        }
    }

    /// 特定のミキサー+リソースタイプの予約数を取得
    pub fn get_mixer(&self, mixer: Entity, resource_type: ResourceType) -> usize {
        self.mixer_reservations.get(&(mixer, resource_type)).cloned().unwrap_or(0)
    }
}

/// タスク状態から搬送予約を同期する
pub fn sync_haul_reservations_system(
    q_souls: Query<&AssignedTask>,
    mut haul_cache: ResMut<HaulReservationCache>,
) {
    let mut reservations: HashMap<Entity, usize> = HashMap::new();
    let mut mixer_reservations: HashMap<(Entity, ResourceType), usize> = HashMap::new();

    for task in q_souls.iter() {
        match task {
            AssignedTask::Haul(data) => {
                *reservations.entry(data.stockpile).or_insert(0) += 1;
            }
            AssignedTask::GatherWater(data) => {
                *reservations.entry(data.tank).or_insert(0) += 1;
            }
            AssignedTask::HaulToMixer(data) => {
                *mixer_reservations.entry((data.mixer, data.resource_type)).or_insert(0) += 1;
            }
            AssignedTask::HaulWaterToMixer(data) => {
                *mixer_reservations
                    .entry((data.mixer, ResourceType::Water))
                    .or_insert(0) += 1;
            }
            _ => {}
        }
    }

    haul_cache.reset(reservations, mixer_reservations);
}

