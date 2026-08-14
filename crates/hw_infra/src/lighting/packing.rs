use super::FieldSnapshot;

pub fn pack_rgba8_linear(snapshot: &FieldSnapshot) -> Vec<u8> {
    let mut packed = Vec::with_capacity(snapshot.cells().len() * 4);
    for (cell, indoor) in snapshot
        .cells()
        .iter()
        .zip(snapshot.indoor_mask_bytes().iter().copied())
    {
        packed.extend_from_slice(&[
            unorm16_to_unorm8(cell.r),
            unorm16_to_unorm8(cell.g),
            unorm16_to_unorm8(cell.b),
            indoor.saturating_mul(u8::MAX),
        ]);
    }
    packed
}

fn unorm16_to_unorm8(value: u16) -> u8 {
    ((u32::from(value) * u32::from(u8::MAX) + 32_767) / u32::from(u16::MAX)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::{
        FixtureMount, GridDimensions, IndoorMask, LightFieldInput, LightGridPos,
        LightOcclusionGrid, LightRadiusTiles, LightRgbLinear, RadialLightEmitterSnapshot,
        rebuild_field,
    };

    #[test]
    fn pack_is_row_major_linear_rgba_with_mask_alpha() {
        let dimensions = GridDimensions::new(2, 1).unwrap();
        let input = LightFieldInput::new(
            dimensions,
            IndoorMask::from_bytes(dimensions, vec![1, 0]).unwrap(),
            LightOcclusionGrid::clear(dimensions),
            vec![RadialLightEmitterSnapshot {
                stable_key: 1,
                mount: FixtureMount::FreeStanding {
                    origin: LightGridPos::new(0, 0),
                },
                radius_tiles: LightRadiusTiles::new(2),
                color: LightRgbLinear {
                    r: u16::MAX,
                    g: 32_768,
                    b: 0,
                },
                intensity: u16::MAX,
            }],
        )
        .unwrap();
        let snapshot = rebuild_field(None, &input).unwrap().snapshot;

        assert_eq!(
            pack_rgba8_linear(&snapshot),
            vec![255, 128, 0, 255, 0, 0, 0, 0]
        );
    }
}
