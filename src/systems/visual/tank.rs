//! タンクの視覚的表示システム

use crate::assets::GameAssets;
use crate::systems::jobs::{Building, BuildingType};
use crate::systems::logistics::Stockpile;
use crate::relationships::StoredItems;
use bevy::prelude::*;

/// タンクの状態に応じて画像を更新するシステム
pub fn update_tank_visual_system(
    game_assets: Res<GameAssets>,
    mut q_tanks: Query<
        (&Building, &Stockpile, Option<&StoredItems>, &mut Sprite),
        With<Building>,
    >,
) {
    for (building, stockpile, stored_items_opt, mut sprite) in q_tanks.iter_mut() {
        // タンクのみ処理
        if building.kind != BuildingType::Tank {
            continue;
        }

        // StoredItemsの長さで現在の水の量を取得
        let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

        // タンクの状態に応じて画像を選択
        let image_handle = if current_count == 0 {
            // 空
            game_assets.tank_empty.clone()
        } else if current_count >= stockpile.capacity {
            // 満タン
            game_assets.tank_full.clone()
        } else {
            // 空でない（満タンでない）
            game_assets.tank_partial.clone()
        };

        sprite.image = image_handle;
    }
}
