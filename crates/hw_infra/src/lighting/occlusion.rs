use super::{GridDimensions, LightFieldError, LightGridPos};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum OcclusionCell {
    Clear = 0,
    ProvisionalWall = 1,
    OpenDoor = 2,
    CompletedWall = 3,
    ClosedDoor = 4,
    LockedDoor = 5,
}

impl OcclusionCell {
    pub const fn blocks_los(self) -> bool {
        matches!(
            self,
            Self::CompletedWall | Self::ClosedDoor | Self::LockedDoor
        )
    }

    pub const fn is_wall_mount_anchor(self) -> bool {
        matches!(self, Self::CompletedWall)
    }

    pub const fn canonical_byte(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndoorMask {
    dimensions: GridDimensions,
    bytes: Box<[u8]>,
}

impl IndoorMask {
    pub fn from_bytes(dimensions: GridDimensions, bytes: Vec<u8>) -> Result<Self, LightFieldError> {
        if bytes.len() != dimensions.area() {
            return Err(LightFieldError::LengthMismatch {
                field: "indoor mask",
                expected: dimensions.area(),
                actual: bytes.len(),
            });
        }
        if let Some((index, value)) = bytes
            .iter()
            .copied()
            .enumerate()
            .find(|(_, value)| !matches!(value, 0 | 1))
        {
            return Err(LightFieldError::InvalidMaskByte { index, value });
        }
        Ok(Self {
            dimensions,
            bytes: bytes.into_boxed_slice(),
        })
    }

    pub fn filled(dimensions: GridDimensions, indoor: bool) -> Self {
        Self {
            dimensions,
            bytes: vec![u8::from(indoor); dimensions.area()].into_boxed_slice(),
        }
    }

    pub const fn dimensions(&self) -> GridDimensions {
        self.dimensions
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn is_indoor(&self, pos: LightGridPos) -> bool {
        self.dimensions
            .index(pos)
            .is_some_and(|index| self.bytes[index] == 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightOcclusionGrid {
    dimensions: GridDimensions,
    cells: Box<[OcclusionCell]>,
}

impl LightOcclusionGrid {
    pub fn from_cells(
        dimensions: GridDimensions,
        cells: Vec<OcclusionCell>,
    ) -> Result<Self, LightFieldError> {
        if cells.len() != dimensions.area() {
            return Err(LightFieldError::LengthMismatch {
                field: "light occlusion grid",
                expected: dimensions.area(),
                actual: cells.len(),
            });
        }
        Ok(Self {
            dimensions,
            cells: cells.into_boxed_slice(),
        })
    }

    pub fn clear(dimensions: GridDimensions) -> Self {
        Self {
            dimensions,
            cells: vec![OcclusionCell::Clear; dimensions.area()].into_boxed_slice(),
        }
    }

    pub const fn dimensions(&self) -> GridDimensions {
        self.dimensions
    }

    pub fn as_cells(&self) -> &[OcclusionCell] {
        &self.cells
    }

    pub fn cell(&self, pos: LightGridPos) -> Option<OcclusionCell> {
        self.dimensions.index(pos).map(|index| self.cells[index])
    }

    pub fn blocks_los_or_oob(&self, pos: LightGridPos) -> bool {
        self.cell(pos).is_none_or(OcclusionCell::blocks_los)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_cells_separate_blockers_from_mount_anchors() {
        assert!(!OcclusionCell::ProvisionalWall.blocks_los());
        assert!(!OcclusionCell::OpenDoor.blocks_los());
        assert!(OcclusionCell::CompletedWall.blocks_los());
        assert!(OcclusionCell::CompletedWall.is_wall_mount_anchor());
        assert!(OcclusionCell::ClosedDoor.blocks_los());
        assert!(!OcclusionCell::ClosedDoor.is_wall_mount_anchor());
        assert!(OcclusionCell::LockedDoor.blocks_los());
        assert!(!OcclusionCell::LockedDoor.is_wall_mount_anchor());
    }

    #[test]
    fn mask_rejects_length_and_non_boolean_bytes() {
        let dimensions = GridDimensions::new(2, 2).unwrap();
        assert!(IndoorMask::from_bytes(dimensions, vec![1, 1, 1]).is_err());
        assert!(IndoorMask::from_bytes(dimensions, vec![1, 1, 1, 2]).is_err());
    }
}
