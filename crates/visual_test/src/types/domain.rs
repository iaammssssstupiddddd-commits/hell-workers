use super::*;

// ─── 表情 ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FaceExpression {
    #[default]
    Normal,
    Fear,
    Exhausted,
    Concentration,
    Happy,
    Sleep,
}

impl FaceExpression {
    pub const ALL: [Self; 6] = [
        Self::Normal,
        Self::Fear,
        Self::Exhausted,
        Self::Concentration,
        Self::Happy,
        Self::Sleep,
    ];

    pub fn uv_offset(self) -> Vec2 {
        let (col, row) = match self {
            Self::Normal => (0.0, 0.0),
            Self::Fear => (1.0, 0.0),
            Self::Exhausted => (2.0, 0.0),
            Self::Concentration => (0.0, 1.0),
            Self::Happy => (1.0, 1.0),
            Self::Sleep => (2.0, 1.0),
        };
        face_uv_offset(col, row)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Fear => "Fear",
            Self::Exhausted => "Exhausted",
            Self::Concentration => "Concentration",
            Self::Happy => "Happy",
            Self::Sleep => "Sleep",
        }
    }
}

// ─── モーション ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionMode {
    #[default]
    Idle,
    FloatingBob,
    Sleeping,
    Resting,
    Escaping,
    Dancing,
}

impl MotionMode {
    pub const ALL: [Self; 6] = [
        Self::Idle,
        Self::FloatingBob,
        Self::Sleeping,
        Self::Resting,
        Self::Escaping,
        Self::Dancing,
    ];

    pub fn next(self) -> Self {
        match self {
            Self::Idle => Self::FloatingBob,
            Self::FloatingBob => Self::Sleeping,
            Self::Sleeping => Self::Resting,
            Self::Resting => Self::Escaping,
            Self::Escaping => Self::Dancing,
            Self::Dancing => Self::Idle,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::FloatingBob => "FloatingBob",
            Self::Sleeping => "Sleeping",
            Self::Resting => "Resting",
            Self::Escaping => "Escaping",
            Self::Dancing => "Dancing",
        }
    }
}

// ─── 矢視方向 ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestElevDir {
    #[default]
    TopDown,
    North,
    East,
    South,
    West,
}

impl TestElevDir {
    pub fn next(self) -> Self {
        match self {
            Self::TopDown => Self::North,
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::TopDown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TopDown => "TopDown",
            Self::North => "North",
            Self::East => "East",
            Self::South => "South",
            Self::West => "West",
        }
    }

    pub fn is_top_down(self) -> bool {
        self == Self::TopDown
    }

    pub fn camera_rotation(self, view_height: f32, z_offset: f32) -> Quat {
        match self {
            Self::TopDown => {
                Transform::from_xyz(0.0, view_height, z_offset)
                    .looking_at(Vec3::ZERO, Vec3::NEG_Z)
                    .rotation
            }
            Self::North => {
                Transform::from_xyz(0.0, 0.0, 1.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
            Self::South => {
                Transform::from_xyz(0.0, 0.0, -1.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
            Self::East => {
                Transform::from_xyz(1.0, 0.0, 0.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
            Self::West => {
                Transform::from_xyz(-1.0, 0.0, 0.0)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct TestElev {
    pub dir: TestElevDir,
}

/// ビジュアルテストの操作モード。[Space] で切替。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppMode {
    #[default]
    Soul,
    Build,
}

impl AppMode {
    pub fn next(self) -> Self {
        match self {
            Self::Soul => Self::Build,
            Self::Build => Self::Soul,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Soul => "SOUL",
            Self::Build => "BUILD",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SoulLayout {
    #[default]
    Default,
    ShadowCompare,
}

impl SoulLayout {
    pub fn toggle(self) -> Self {
        match self {
            Self::Default => Self::ShadowCompare,
            Self::ShadowCompare => Self::Default,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::ShadowCompare => "A/B  L=GLB R=Blob",
        }
    }
}

/// ビジュアルテスト内で建築物種別を選択するためのローカル enum。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestBuildingKind {
    #[default]
    Wall,
    Door,
    Floor,
    Tank,
    MudMixer,
    RestArea,
    Bridge,
    SandPile,
    BonePile,
    WheelbarrowParking,
    SoulSpa,
}

impl TestBuildingKind {
    pub const ALL: [Self; 11] = [
        Self::Wall,
        Self::Door,
        Self::Floor,
        Self::Tank,
        Self::MudMixer,
        Self::RestArea,
        Self::Bridge,
        Self::SandPile,
        Self::BonePile,
        Self::WheelbarrowParking,
        Self::SoulSpa,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Door => "Door",
            Self::Floor => "Floor",
            Self::Tank => "Tank",
            Self::MudMixer => "MudMixer",
            Self::RestArea => "RestArea",
            Self::Bridge => "Bridge",
            Self::SandPile => "SandPile",
            Self::BonePile => "BonePile",
            Self::WheelbarrowParking => "WheelbarrowParking",
            Self::SoulSpa => "SoulSpa",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&v| v == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}
