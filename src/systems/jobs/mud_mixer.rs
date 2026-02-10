//! MudMixer 関連の定義とロジック
use crate::constants::MUD_MIXER_CAPACITY;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component, Default)]
pub struct MudMixerStorage {
    pub sand: u32,
    pub rock: u32,
}

/// ミキサーを対象としたターゲットマーカー
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TargetMixer(pub Entity);


impl MudMixerStorage {
    /// 指定されたリソースが満杯かチェック
    pub fn is_full(&self, resource: ResourceType) -> bool {
        match resource {
            ResourceType::Sand => self.sand >= MUD_MIXER_CAPACITY,
            ResourceType::Rock => self.rock >= MUD_MIXER_CAPACITY,
            ResourceType::Water => false,
            _ => true, // 他のリソースは受け入れない
        }
    }

    /// 指定された量のリソースを受け入れ可能かチェック（現在の在庫 + 追加分 <= キャパシティ）
    pub fn can_accept(&self, resource: ResourceType, amount: u32) -> bool {
        match resource {
            ResourceType::Sand => self.sand + amount <= MUD_MIXER_CAPACITY,
            ResourceType::Rock => self.rock + amount <= MUD_MIXER_CAPACITY,
            ResourceType::Water => false, // 水は Stockpile で管理
            _ => false,
        }
    }

    /// リソースを指定量追加する。実際に加算された量を返す。
    pub fn add_amount(&mut self, resource: ResourceType, amount: u32) -> u32 {
        let capacity = MUD_MIXER_CAPACITY;
        match resource {
            ResourceType::Sand => {
                let current = self.sand;
                let to_add = amount.min(capacity.saturating_sub(current));
                self.sand += to_add;
                to_add
            }
            ResourceType::Rock => {
                let current = self.rock;
                let to_add = amount.min(capacity.saturating_sub(current));
                self.rock += to_add;
                to_add
            }
            _ => 0,
        }
    }

    /// リソースを1つ追加する。成功した場合は Ok(())
    pub fn add_material(&mut self, resource: ResourceType) -> Result<(), ()> {
        if self.add_amount(resource, 1) == 1 {
            Ok(())
        } else {
            Err(())
        }
    }

    /// 精製（Refine）に必要な素材が揃っているか確認
    pub fn has_materials_for_refining(&self, water_count: u32) -> bool {
        self.sand >= 1 && water_count >= 1 && self.rock >= 1
    }

    /// 素材を消費して精製を開始する。成功した場合は Ok(())
    pub fn consume_materials_for_refining(&mut self, water_count: u32) -> Result<(), ()> {
        if !self.has_materials_for_refining(water_count) {
            return Err(());
        }

        self.sand = self.sand.saturating_sub(1);
        self.rock = self.rock.saturating_sub(1);
        Ok(())
    }
}
