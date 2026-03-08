//! MudMixer 関連の定義とロジック

use bevy::prelude::*;

use hw_core::constants::{MUD_MIXER_CAPACITY, MUD_MIXER_MUD_CAPACITY, STASIS_MUD_OUTPUT};
use hw_core::logistics::ResourceType;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component, Default)]
pub struct MudMixerStorage {
    pub sand: u32,
    pub rock: u32,
    pub mud: u32,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TargetMixer(pub Entity);

#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct StoredByMixer(pub Entity);

impl MudMixerStorage {
    pub fn is_full(&self, resource: ResourceType) -> bool {
        match resource {
            ResourceType::Sand => self.sand >= MUD_MIXER_CAPACITY,
            ResourceType::Rock => self.rock >= MUD_MIXER_CAPACITY,
            ResourceType::Water => false,
            _ => true,
        }
    }

    pub fn can_accept(&self, resource: ResourceType, amount: u32) -> bool {
        match resource {
            ResourceType::Sand => self.sand + amount <= MUD_MIXER_CAPACITY,
            ResourceType::Rock => self.rock + amount <= MUD_MIXER_CAPACITY,
            ResourceType::Water => false,
            _ => false,
        }
    }

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

    pub fn add_material(&mut self, resource: ResourceType) -> Result<(), ()> {
        if self.add_amount(resource, 1) == 1 {
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn has_materials_for_refining(&self, water_count: u32) -> bool {
        self.sand >= 1 && water_count >= 1 && self.rock >= 1
    }

    pub fn has_output_capacity_for_refining(&self) -> bool {
        self.mud + STASIS_MUD_OUTPUT <= MUD_MIXER_MUD_CAPACITY
    }

    pub fn consume_materials_for_refining(&mut self, water_count: u32) -> Result<(), ()> {
        if !self.has_materials_for_refining(water_count) || !self.has_output_capacity_for_refining()
        {
            return Err(());
        }

        self.sand = self.sand.saturating_sub(1);
        self.rock = self.rock.saturating_sub(1);
        Ok(())
    }
}
