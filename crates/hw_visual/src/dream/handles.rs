use super::dream_bubble_material::DreamBubbleMaterial;
use bevy::prelude::*;
use hw_core::soul::DreamQuality;

pub const ALPHA_BUCKETS: usize = 8;

#[derive(Resource)]
pub struct DreamBubbleHandles {
    pub circle_mesh: Handle<Mesh>,
    /// materials[quality_index][alpha_bucket]
    /// quality_index: 0=VividDream, 1=NormalDream, 2=NightTerror
    pub materials: [[Handle<DreamBubbleMaterial>; ALPHA_BUCKETS]; 3],
}

const QUALITY_COLORS: [(DreamQuality, LinearRgba); 3] = [
    (DreamQuality::VividDream, LinearRgba::new(0.55, 0.80, 1.00, 1.0)),
    (DreamQuality::NormalDream, LinearRgba::new(0.55, 0.65, 0.95, 1.0)),
    (DreamQuality::NightTerror, LinearRgba::new(0.95, 0.45, 0.55, 1.0)),
];

pub fn init_dream_bubble_handles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DreamBubbleMaterial>>,
) {
    let circle_mesh = meshes.add(Circle::new(0.5));

    let pool = std::array::from_fn(|qi| {
        let color = QUALITY_COLORS[qi].1;
        std::array::from_fn(|b| {
            let alpha = b as f32 / (ALPHA_BUCKETS as f32 - 1.0) * 0.85;
            materials.add(DreamBubbleMaterial {
                color,
                alpha,
                mass: 1.0,
            })
        })
    });

    commands.insert_resource(DreamBubbleHandles {
        circle_mesh,
        materials: pool,
    });
}

/// `DreamQuality` を quality_index (0–2) に変換する
pub fn quality_index(q: DreamQuality) -> usize {
    match q {
        DreamQuality::VividDream => 0,
        DreamQuality::NormalDream => 1,
        DreamQuality::NightTerror => 2,
        DreamQuality::Awake => 0, // スポーン前にガードされるが安全側
    }
}

/// `life_ratio` (0.0–1.0) から alpha_bucket (0–7) を算出する
pub fn life_ratio_to_bucket(life_ratio: f32) -> usize {
    ((life_ratio * ALPHA_BUCKETS as f32).floor() as usize).min(ALPHA_BUCKETS - 1)
}
