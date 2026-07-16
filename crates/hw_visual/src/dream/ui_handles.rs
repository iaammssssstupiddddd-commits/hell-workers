use super::dream_bubble_material::DreamBubbleUiMaterial;
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

pub const UI_ALPHA_BUCKETS: usize = 8;
pub const UI_MASS_BUCKETS: usize = 4;
pub const UI_COLOR_BUCKETS: usize = 2;

const UI_BUCKET_COUNT: usize = UI_ALPHA_BUCKETS * UI_MASS_BUCKETS * UI_COLOR_BUCKETS;

const NORMAL_COLOR: LinearRgba = LinearRgba::new(0.65, 0.9, 1.0, 1.0);
const NEAR_WHITE_COLOR: LinearRgba = LinearRgba::new(1.0, 1.0, 1.0, 1.0);

#[derive(Resource)]
pub struct DreamBubbleUiHandles {
    pub materials: Vec<Handle<DreamBubbleUiMaterial>>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
pub struct DreamUiMaterialBucket {
    pub alpha: u8,
    pub mass: u8,
    pub color: u8,
}

pub fn init_dream_bubble_ui_handles(
    mut commands: Commands,
    mut materials: ResMut<Assets<DreamBubbleUiMaterial>>,
) {
    let mut pool = Vec::with_capacity(UI_BUCKET_COUNT);
    for alpha_bucket in 0..UI_ALPHA_BUCKETS {
        let alpha = alpha_bucket as f32 / (UI_ALPHA_BUCKETS as f32 - 1.0) * 0.9;
        for mass_bucket in 0..UI_MASS_BUCKETS {
            let mass = mass_for_bucket(mass_bucket);
            for color_bucket in 0..UI_COLOR_BUCKETS {
                let color = color_for_bucket(color_bucket);
                pool.push(materials.add(DreamBubbleUiMaterial { color, alpha, mass }));
            }
        }
    }

    commands.insert_resource(DreamBubbleUiHandles { materials: pool });
}

pub fn alpha_to_bucket(alpha: f32) -> u8 {
    ((alpha / 0.9 * UI_ALPHA_BUCKETS as f32).floor() as usize).min(UI_ALPHA_BUCKETS - 1) as u8
}

pub fn mass_to_bucket(mass: f32) -> u8 {
    if mass >= 6.0 {
        3
    } else if mass >= 3.0 {
        2
    } else if mass >= 1.5 {
        1
    } else {
        0
    }
}

pub fn color_to_bucket(visual_distance_ratio: f32) -> u8 {
    if visual_distance_ratio < 0.3 { 1 } else { 0 }
}

pub fn bucket_material_index(bucket: DreamUiMaterialBucket) -> usize {
    let alpha = bucket.alpha as usize;
    let mass = bucket.mass as usize;
    let color = bucket.color as usize;
    (alpha * UI_MASS_BUCKETS + mass) * UI_COLOR_BUCKETS + color
}

pub fn apply_ui_material_bucket(
    mat_node: &mut MaterialNode<DreamBubbleUiMaterial>,
    current: &mut DreamUiMaterialBucket,
    desired: DreamUiMaterialBucket,
    handles: &DreamBubbleUiHandles,
) {
    if *current == desired {
        return;
    }
    let index = bucket_material_index(desired);
    if let Some(handle) = handles.materials.get(index) {
        mat_node.0 = handle.clone();
        *current = desired;
    }
}

fn mass_for_bucket(bucket: usize) -> f32 {
    match bucket {
        0 => 0.5,
        1 => 1.5,
        2 => 3.5,
        _ => 7.0,
    }
}

fn color_for_bucket(bucket: usize) -> LinearRgba {
    if bucket == 1 {
        NEAR_WHITE_COLOR
    } else {
        NORMAL_COLOR
    }
}
