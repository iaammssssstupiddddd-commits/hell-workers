//! Producer upsert/cleanup 共通化ヘルパー
//!
//! Phase 1: 重複 request 排除、需要ゼロ時の disable/despawn、existing 更新 / new spawn
//! の共通パターンを提供する。各 producer は需要計算に集中し、このヘルパーで
//! 実際の upsert/cleanup を行う。

use bevy::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;

use crate::systems::jobs::{Designation, Priority, TaskSlots};

/// 既存 request のループで重複を検出した際の処理
/// 戻り値: 続行するか (false = continue でスキップ)
#[inline]
pub fn process_duplicate_key<K: Hash + Eq>(
    commands: &mut Commands,
    entity: Entity,
    workers: usize,
    seen: &mut HashSet<K>,
    key: K,
) -> bool {
    if !seen.insert(key) {
        if workers == 0 {
            commands.entity(entity).despawn();
        }
        return false;
    }
    true
}

/// 需要なし時の disable（Designation/TaskSlots/Priority を remove）
/// 需要がなくなったがアンカーは存在する場合に呼ぶ
#[inline]
pub fn disable_request(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Designation>()
        .remove::<TaskSlots>()
        .remove::<Priority>();
}
