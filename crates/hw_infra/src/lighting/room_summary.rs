use super::{FieldSampleError, LightCell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomIlluminationSummary {
    pub sample_count: u32,
    pub dark_cells: u32,
    pub mean_luminance: u16,
    pub min_luminance: u16,
    pub dark_ratio_q16: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomSummaryError {
    Empty,
    InvalidSample(FieldSampleError),
    TooManySamples,
}

pub fn summarize_room_illumination<I>(
    samples: I,
) -> Result<RoomIlluminationSummary, RoomSummaryError>
where
    I: IntoIterator<Item = Result<LightCell, FieldSampleError>>,
{
    let mut sample_count = 0_u32;
    let mut dark_cells = 0_u32;
    let mut luminance_sum = 0_u64;
    let mut min_luminance = u16::MAX;

    for sample in samples {
        let cell = sample.map_err(RoomSummaryError::InvalidSample)?;
        sample_count = sample_count
            .checked_add(1)
            .ok_or(RoomSummaryError::TooManySamples)?;
        dark_cells = dark_cells.saturating_add(u32::from(cell.luminance == 0));
        luminance_sum = luminance_sum.saturating_add(u64::from(cell.luminance));
        min_luminance = min_luminance.min(cell.luminance);
    }

    if sample_count == 0 {
        return Err(RoomSummaryError::Empty);
    }

    let count = u64::from(sample_count);
    let mean_luminance = ((luminance_sum + count / 2) / count) as u16;
    let dark_ratio_q16 = ((u64::from(dark_cells) * u64::from(u16::MAX) + count / 2) / count) as u16;

    Ok(RoomIlluminationSummary {
        sample_count,
        dark_cells,
        mean_luminance,
        min_luminance,
        dark_ratio_q16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::LightGridPos;

    fn cell(luminance: u16) -> Result<LightCell, FieldSampleError> {
        Ok(LightCell {
            luminance,
            ..LightCell::default()
        })
    }

    #[test]
    fn fixed_vector_uses_integer_rounding_and_q16_ratio() {
        let summary = summarize_room_illumination([cell(0), cell(1), cell(2)]).unwrap();
        assert_eq!(
            summary,
            RoomIlluminationSummary {
                sample_count: 3,
                dark_cells: 1,
                mean_luminance: 1,
                min_luminance: 0,
                dark_ratio_q16: 21_845,
            }
        );
    }

    #[test]
    fn empty_and_invalid_inputs_fail_closed() {
        assert_eq!(
            summarize_room_illumination(std::iter::empty()),
            Err(RoomSummaryError::Empty)
        );
        let pos = LightGridPos::new(9, 4);
        assert_eq!(
            summarize_room_illumination([Err(FieldSampleError::OutsideIndoorMask(pos))]),
            Err(RoomSummaryError::InvalidSample(
                FieldSampleError::OutsideIndoorMask(pos)
            ))
        );
    }
}
