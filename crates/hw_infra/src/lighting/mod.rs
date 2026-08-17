mod field;
mod fixture;
mod los;
mod occlusion;
mod packing;
mod room_summary;

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use serde::{Deserialize, Serialize};
use std::fmt;

pub use field::{
    EmitterDiagnostic, EmitterDiagnosticReason, FieldSampleError, FieldSnapshot, LightCell,
    LightFieldInput, RebuildOutcome, Sha256Digest, digest_hex, rebuild_field,
};
pub use fixture::canonical_large_field_input;
pub use los::has_line_of_sight;
pub use occlusion::{IndoorMask, LightOcclusionGrid, OcclusionCell};
pub use packing::pack_rgba8_linear;
pub use room_summary::{RoomIlluminationSummary, RoomSummaryError, summarize_room_illumination};

pub const MAX_GRID_CELLS: usize = (MAP_WIDTH as usize) * (MAP_HEIGHT as usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridDimensions {
    width: u16,
    height: u16,
}

impl GridDimensions {
    pub fn new(width: u16, height: u16) -> Result<Self, LightFieldError> {
        let width_limit = u16::try_from(MAP_WIDTH)
            .map_err(|_| LightFieldError::InvalidDimensions { width, height })?;
        let height_limit = u16::try_from(MAP_HEIGHT)
            .map_err(|_| LightFieldError::InvalidDimensions { width, height })?;
        if width == 0 || height == 0 || width > width_limit || height > height_limit {
            return Err(LightFieldError::InvalidDimensions { width, height });
        }
        let area = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or(LightFieldError::DimensionOverflow)?;
        if area > MAX_GRID_CELLS {
            return Err(LightFieldError::InvalidDimensions { width, height });
        }
        Ok(Self { width, height })
    }

    pub const fn width(self) -> u16 {
        self.width
    }

    pub const fn height(self) -> u16 {
        self.height
    }

    pub fn area(self) -> usize {
        usize::from(self.width) * usize::from(self.height)
    }

    pub fn contains(self, pos: LightGridPos) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < i32::from(self.width) && pos.y < i32::from(self.height)
    }

    pub fn index(self, pos: LightGridPos) -> Option<usize> {
        self.contains(pos).then(|| {
            usize::try_from(pos.y).expect("non-negative grid y") * usize::from(self.width)
                + usize::try_from(pos.x).expect("non-negative grid x")
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LightGridPos {
    pub x: i32,
    pub y: i32,
}

impl LightGridPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub const fn offset(self, delta: (i32, i32)) -> Self {
        Self {
            x: self.x + delta.0,
            y: self.y + delta.1,
        }
    }

    pub const fn into_core(self) -> hw_core::GridPos {
        (self.x, self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LightRadiusTiles(u16);

impl LightRadiusTiles {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum CardinalDirection {
    North = 0,
    East = 1,
    South = 2,
    West = 3,
}

impl CardinalDirection {
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Self::North => (0, 1),
            Self::East => (1, 0),
            Self::South => (0, -1),
            Self::West => (-1, 0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FixtureMount {
    FreeStanding {
        origin: LightGridPos,
    },
    WallMounted {
        anchor: LightGridPos,
        inward: CardinalDirection,
    },
}

impl FixtureMount {
    pub const fn origin(self) -> LightGridPos {
        match self {
            Self::FreeStanding { origin } => origin,
            Self::WallMounted { anchor, inward } => anchor.offset(inward.delta()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LightRgbLinear {
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

impl LightRgbLinear {
    pub const WHITE: Self = Self {
        r: u16::MAX,
        g: u16::MAX,
        b: u16::MAX,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RadialLightEmitterSnapshot {
    pub stable_key: u64,
    pub mount: FixtureMount,
    pub radius_tiles: LightRadiusTiles,
    pub color: LightRgbLinear,
    pub intensity: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightFieldError {
    InvalidDimensions {
        width: u16,
        height: u16,
    },
    DimensionOverflow,
    LengthMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    DimensionMismatch {
        field: &'static str,
        expected: GridDimensions,
        actual: GridDimensions,
    },
    InvalidMaskByte {
        index: usize,
        value: u8,
    },
    DuplicateEmitterKey(u64),
    RevisionOverflow,
}

impl fmt::Display for LightFieldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => {
                write!(formatter, "invalid light grid dimensions {width}x{height}")
            }
            Self::DimensionOverflow => write!(formatter, "light grid area overflow"),
            Self::LengthMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} length mismatch: expected {expected}, got {actual}"
            ),
            Self::DimensionMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} dimensions mismatch: expected {}x{}, got {}x{}",
                expected.width(),
                expected.height(),
                actual.width(),
                actual.height()
            ),
            Self::InvalidMaskByte { index, value } => {
                write!(
                    formatter,
                    "invalid indoor mask byte {value} at index {index}"
                )
            }
            Self::DuplicateEmitterKey(key) => {
                write!(formatter, "duplicate emitter stable key {key}")
            }
            Self::RevisionOverflow => write!(formatter, "light field revision overflow"),
        }
    }
}

impl std::error::Error for LightFieldError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_reject_zero_and_world_overflow() {
        assert!(GridDimensions::new(0, 1).is_err());
        assert!(GridDimensions::new(1, 0).is_err());
        assert!(GridDimensions::new(101, 100).is_err());
        assert!(GridDimensions::new(100, 101).is_err());
        assert_eq!(GridDimensions::new(100, 100).unwrap().area(), 10_000);
    }

    #[test]
    fn wall_mount_origin_is_exactly_one_cardinal_cell_inward() {
        let mount = FixtureMount::WallMounted {
            anchor: LightGridPos::new(5, 7),
            inward: CardinalDirection::South,
        };
        assert_eq!(mount.origin(), LightGridPos::new(5, 6));
    }
}
