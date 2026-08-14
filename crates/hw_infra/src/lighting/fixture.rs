use std::collections::BTreeSet;

use super::{
    FixtureMount, GridDimensions, IndoorMask, LightFieldError, LightFieldInput, LightGridPos,
    LightOcclusionGrid, LightRadiusTiles, LightRgbLinear, OcclusionCell,
    RadialLightEmitterSnapshot,
};

const ORIGIN: LightGridPos = LightGridPos::new(16, 20);
const MODULE_COUNT: i32 = 4;
const MODULE_EXTENT: i32 = 7;
const SUPPLIED_EMITTERS: usize = 50;

pub fn canonical_large_field_input() -> Result<LightFieldInput, LightFieldError> {
    let dimensions = GridDimensions::new(100, 100)?;
    let floors = floor_cells();
    let mut mask = vec![0_u8; dimensions.area()];
    for &floor in &floors {
        mask[dimensions
            .index(floor)
            .expect("canonical floor must fit the 100x100 grid")] = 1;
    }

    let mut occlusion = vec![OcclusionCell::Clear; dimensions.area()];
    for wall in boundary_cells() {
        occlusion[dimensions
            .index(wall)
            .expect("canonical wall must fit the 100x100 grid")] = OcclusionCell::CompletedWall;
    }
    for (ordinal, door) in door_cells().into_iter().enumerate() {
        occlusion[dimensions
            .index(door)
            .expect("canonical door must fit the 100x100 grid")] = match ordinal {
            0..=3 => OcclusionCell::OpenDoor,
            4..=11 => OcclusionCell::ClosedDoor,
            _ => OcclusionCell::LockedDoor,
        };
    }

    let emitters = floors
        .iter()
        .copied()
        .take(SUPPLIED_EMITTERS)
        .enumerate()
        .map(|(ordinal, origin)| RadialLightEmitterSnapshot {
            stable_key: u64::try_from(ordinal + 1).expect("canonical emitter ordinal fits u64"),
            mount: FixtureMount::FreeStanding { origin },
            radius_tiles: LightRadiusTiles::new(5),
            color: LightRgbLinear {
                r: u16::MAX,
                g: 57_344,
                b: 45_056,
            },
            intensity: u16::MAX,
        })
        .collect();

    LightFieldInput::new(
        dimensions,
        IndoorMask::from_bytes(dimensions, mask)?,
        LightOcclusionGrid::from_cells(dimensions, occlusion)?,
        emitters,
    )
}

fn floor_cells() -> Vec<LightGridPos> {
    let mut floors = Vec::with_capacity(576);
    for room_y in 0..MODULE_COUNT {
        for room_x in 0..MODULE_COUNT {
            for local_y in 1..=6 {
                for local_x in 1..=6 {
                    floors.push(LightGridPos::new(
                        ORIGIN.x + room_x * MODULE_EXTENT + local_x,
                        ORIGIN.y + room_y * MODULE_EXTENT + local_y,
                    ));
                }
            }
        }
    }
    floors
}

fn boundary_cells() -> BTreeSet<LightGridPos> {
    let extent = MODULE_COUNT * MODULE_EXTENT;
    let mut boundary = BTreeSet::new();
    for boundary_index in 0..=MODULE_COUNT {
        let line = boundary_index * MODULE_EXTENT;
        for offset in 0..=extent {
            boundary.insert(LightGridPos::new(ORIGIN.x + line, ORIGIN.y + offset));
            boundary.insert(LightGridPos::new(ORIGIN.x + offset, ORIGIN.y + line));
        }
    }
    boundary
}

fn door_cells() -> Vec<LightGridPos> {
    let mut doors = Vec::with_capacity(16);
    for room_y in 0..MODULE_COUNT {
        for room_x in 0..MODULE_COUNT {
            doors.push(LightGridPos::new(
                ORIGIN.x + room_x * MODULE_EXTENT + 3,
                ORIGIN.y + (room_y + 1) * MODULE_EXTENT,
            ));
        }
    }
    doors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::{digest_hex, rebuild_field};

    #[test]
    fn canonical_large_fixture_matches_the_frozen_field_core_shape() {
        let input = canonical_large_field_input().unwrap();
        let outcome = rebuild_field(None, &input).unwrap();

        assert_eq!(input.dimensions().area(), 10_000);
        assert_eq!(
            input
                .indoor_mask()
                .as_bytes()
                .iter()
                .map(|&byte| usize::from(byte))
                .sum::<usize>(),
            576
        );
        assert_eq!(input.emitters().len(), 50);
        assert_eq!(outcome.snapshot.logical_payload_bytes(), 80_000);
        assert!(outcome.diagnostics.is_empty());
        assert_eq!(outcome.snapshot.changed_mask_cells(), 576);
        assert_eq!(outcome.snapshot.changed_cell_count(), 576);
    }

    #[test]
    fn canonical_large_fixture_has_stable_input_and_output_checksums() {
        let outcome = rebuild_field(None, &canonical_large_field_input().unwrap()).unwrap();
        assert_eq!(
            digest_hex(&outcome.input_checksum),
            "3b6c33a4381e38eb7804594f0da062911682b337c6d6429a03ecf3fb0e9e0412"
        );
        assert_eq!(
            digest_hex(outcome.snapshot.field_checksum()),
            "d22ce795eb94f56bea413dfa1c8d10b5b021f9ea1a138bcab91584c24761e8df"
        );
    }
}
