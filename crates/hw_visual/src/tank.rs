//! タンクの視覚的表示システム

use crate::handles::BuildingAnimHandles;
use crate::layer::VisualLayerKind;
use bevy::prelude::*;
use hw_core::relationships::StoredItems;
use hw_core::visual_mirror::StockpileVisualState;
use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};

/// タンクの状態に応じて画像を更新するシステム
pub fn update_tank_visual_system(
    handles: Res<BuildingAnimHandles>,
    q_tanks: Query<(
        Entity,
        &BuildingVisualState,
        &StockpileVisualState,
        Option<&StoredItems>,
    )>,
    q_children: Query<&Children>,
    mut q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
) {
    for (entity, building_visual, stockpile_visual, stored_items_opt) in q_tanks.iter() {
        if building_visual.kind != BuildingTypeVisual::Tank {
            continue;
        }

        let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

        let image_handle = if current_count == 0 {
            handles.tank_empty.clone()
        } else if current_count >= stockpile_visual.capacity {
            handles.tank_full.clone()
        } else {
            handles.tank_partial.clone()
        };

        if let Ok(children) = q_children.get(entity) {
            for child in children.iter() {
                if let Ok((kind, mut sprite)) = q_visual_layers.get_mut(child) {
                    if *kind == VisualLayerKind::Struct {
                        sprite.image = image_handle;
                        break;
                    }
                }
            }
        }
    }
}
