use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use std::collections::HashMap;

/// システム全体で共有されるリソース予約キャッシュ
///
/// タスク間（運搬、建築、採集など）でのリソース競合を防ぐために使用される。
/// Senseフェーズで再構築され、Thinkフェーズで仮予約、Actフェーズで確定更新される。
#[derive(Resource, Default, Debug)]
pub struct SharedResourceCache {
    /// ミキサー等への予約数 (Destination Reservation)
    /// (Entity, ResourceType) -> 搬送予定数
    mixer_dest_reservations: HashMap<(Entity, ResourceType), usize>,

    /// リソース/タンクからの取り出し予約数 (Source Reservation)
    /// Entity -> 取り出し予定数
    source_reservations: HashMap<Entity, usize>,

    /// このフレームで格納された数（コンポーネント未反映分）
    /// Entity -> 格納数
    frame_stored_count: HashMap<Entity, usize>,

    /// このフレームで取り出された数（コンポーネント未反映分）
    /// Entity -> 取り出し数
    frame_picked_count: HashMap<Entity, usize>,
}

impl SharedResourceCache {
    /// 外部から予約状況を再構築する（Senseフェーズ用）
    pub fn reset(
        &mut self,
        mixer_dest_reservations: HashMap<(Entity, ResourceType), usize>,
        source_reservations: HashMap<Entity, usize>,
    ) {
        self.mixer_dest_reservations = mixer_dest_reservations;
        self.source_reservations = source_reservations;
        self.frame_stored_count.clear();
        self.frame_picked_count.clear();
    }

    /// ミキサーへの予約を追加
    pub fn reserve_mixer_destination(&mut self, target: Entity, resource_type: ResourceType) {
        *self
            .mixer_dest_reservations
            .entry((target, resource_type))
            .or_insert(0) += 1;
    }

    /// ミキサーへの予約を解除
    pub fn release_mixer_destination(&mut self, target: Entity, resource_type: ResourceType) {
        let key = (target, resource_type);
        if let Some(count) = self.mixer_dest_reservations.get_mut(&key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.mixer_dest_reservations.remove(&key);
            }
        }
    }

    /// ミキサーへの予約数を取得
    pub fn get_mixer_destination_reservation(
        &self,
        target: Entity,
        resource_type: ResourceType,
    ) -> usize {
        self.mixer_dest_reservations
            .get(&(target, resource_type))
            .cloned()
            .unwrap_or(0)
    }

    /// リソース取り出し予約を追加 (Source Reservation)
    pub fn reserve_source(&mut self, source: Entity, amount: usize) {
        *self.source_reservations.entry(source).or_insert(0) += amount;
    }

    /// リソース取り出し予約を解除
    pub fn release_source(&mut self, source: Entity, amount: usize) {
        if let Some(count) = self.source_reservations.get_mut(&source) {
            *count = count.saturating_sub(amount);
            if *count == 0 {
                self.source_reservations.remove(&source);
            }
        }
    }

    /// リソース取り出し予約数を取得（予約済み + このフレームで取得済み）
    pub fn get_source_reservation(&self, source: Entity) -> usize {
        let reserved = self.source_reservations.get(&source).cloned().unwrap_or(0);
        let picked = self.frame_picked_count.get(&source).cloned().unwrap_or(0);
        reserved + picked
    }

    /// 取得アクション成功を記録 (Delta Update)
    /// ソース予約を減らし、フレーム内取得数を増やす（論理在庫減少）
    pub fn record_picked_source(&mut self, source: Entity, amount: usize) {
        self.release_source(source, amount);
        *self.frame_picked_count.entry(source).or_insert(0) += amount;
    }

    /// 論理的な格納済み数（クエリ値 + フレーム内増加分）を取得
    pub fn get_logical_stored_count(&self, target: Entity, current_from_query: usize) -> usize {
        let frame_added = self.frame_stored_count.get(&target).cloned().unwrap_or(0);
        current_from_query + frame_added
    }
}
