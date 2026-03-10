//! タンクの視覚的表示システム

use crate::handles::BuildingAnimHandles;
use bevy::prelude::*;
use hw_core::relationships::StoredItems;
use hw_jobs::{Building, BuildingType};
use hw_logistics::zone::Stockpile;

/// タンクの状態に応じて画像を更新するシステム
pub fn update_tank_visual_system(
    handles: Res<BuildingAnimHandles>,
    mut q_tanks: Query<(&Building, &Stockpile, Option<&StoredItems>, &mut Sprite), With<Building>>,
) {
    for (building, stockpile, stored_items_opt, mut sprite) in q_tanks.iter_mut() {
        if building.kind != BuildingType::Tank {
            continue;
        }

        let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

        let image_handle = if current_count == 0 {
            handles.tank_empty.clone()
        } else if current_count >= stockpile.capacity {
            handles.tank_full.clone()
        } else {
            handles.tank_partial.clone()
        };

        sprite.image = image_handle;
    }
}
