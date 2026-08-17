use super::{
    FixtureMount, GridDimensions, IndoorMask, LightFieldError, LightGridPos, LightOcclusionGrid,
    RadialLightEmitterSnapshot, has_line_of_sight,
};
use sha2::{Digest, Sha256};

pub type Sha256Digest = [u8; 32];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LightCell {
    pub r: u16,
    pub g: u16,
    pub b: u16,
    pub luminance: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSampleError {
    OutOfBounds(LightGridPos),
    OutsideIndoorMask(LightGridPos),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightFieldInput {
    dimensions: GridDimensions,
    indoor_mask: IndoorMask,
    occlusion: LightOcclusionGrid,
    emitters: Vec<RadialLightEmitterSnapshot>,
}

impl LightFieldInput {
    pub fn new(
        dimensions: GridDimensions,
        indoor_mask: IndoorMask,
        occlusion: LightOcclusionGrid,
        emitters: Vec<RadialLightEmitterSnapshot>,
    ) -> Result<Self, LightFieldError> {
        if indoor_mask.dimensions() != dimensions {
            return Err(LightFieldError::DimensionMismatch {
                field: "indoor mask",
                expected: dimensions,
                actual: indoor_mask.dimensions(),
            });
        }
        if occlusion.dimensions() != dimensions {
            return Err(LightFieldError::DimensionMismatch {
                field: "light occlusion grid",
                expected: dimensions,
                actual: occlusion.dimensions(),
            });
        }
        Ok(Self {
            dimensions,
            indoor_mask,
            occlusion,
            emitters,
        })
    }

    pub const fn dimensions(&self) -> GridDimensions {
        self.dimensions
    }

    pub const fn indoor_mask(&self) -> &IndoorMask {
        &self.indoor_mask
    }

    pub const fn occlusion(&self) -> &LightOcclusionGrid {
        &self.occlusion
    }

    pub fn emitters(&self) -> &[RadialLightEmitterSnapshot] {
        &self.emitters
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSnapshot {
    dimensions: GridDimensions,
    cells: Box<[LightCell]>,
    indoor_mask: Box<[u8]>,
    field_revision: u64,
    radiance_checksum: Sha256Digest,
    mask_checksum: Sha256Digest,
    field_checksum: Sha256Digest,
    changed_radiance_cells: u32,
    changed_mask_cells: u32,
    changed_cell_count: u32,
}

impl FieldSnapshot {
    pub const fn dimensions(&self) -> GridDimensions {
        self.dimensions
    }

    pub fn cells(&self) -> &[LightCell] {
        &self.cells
    }

    pub fn indoor_mask_bytes(&self) -> &[u8] {
        &self.indoor_mask
    }

    /// Samples gameplay luminance without clamping. Cells outside the indoor
    /// mask are deliberately exposed as dark, while map OOB stays distinct.
    pub fn sample_luminance(&self, pos: LightGridPos) -> Option<u16> {
        let index = self.dimensions.index(pos)?;
        if self.indoor_mask[index] == 0 {
            return Some(0);
        }
        self.cells.get(index).map(|cell| cell.luminance)
    }

    /// Strict Room sampling keeps a malformed Room tile distinguishable from
    /// a valid indoor tile whose luminance is zero.
    pub fn sample_room_cell(&self, pos: LightGridPos) -> Result<LightCell, FieldSampleError> {
        let index = self
            .dimensions
            .index(pos)
            .ok_or(FieldSampleError::OutOfBounds(pos))?;
        if self.indoor_mask[index] == 0 {
            return Err(FieldSampleError::OutsideIndoorMask(pos));
        }
        Ok(self.cells[index])
    }

    pub fn radiance_payload_le(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(self.logical_payload_bytes());
        for cell in &self.cells {
            payload.extend_from_slice(&cell.r.to_le_bytes());
            payload.extend_from_slice(&cell.g.to_le_bytes());
            payload.extend_from_slice(&cell.b.to_le_bytes());
            payload.extend_from_slice(&cell.luminance.to_le_bytes());
        }
        payload
    }

    pub const fn field_revision(&self) -> u64 {
        self.field_revision
    }

    pub const fn radiance_checksum(&self) -> &Sha256Digest {
        &self.radiance_checksum
    }

    pub const fn mask_checksum(&self) -> &Sha256Digest {
        &self.mask_checksum
    }

    pub const fn field_checksum(&self) -> &Sha256Digest {
        &self.field_checksum
    }

    pub const fn changed_radiance_cells(&self) -> u32 {
        self.changed_radiance_cells
    }

    pub const fn changed_mask_cells(&self) -> u32 {
        self.changed_mask_cells
    }

    pub const fn changed_cell_count(&self) -> u32 {
        self.changed_cell_count
    }

    pub fn logical_payload_bytes(&self) -> usize {
        self.cells.len() * 8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterDiagnostic {
    pub stable_key: u64,
    pub reason: EmitterDiagnosticReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitterDiagnosticReason {
    ZeroRadius,
    OriginOutOfBounds,
    FreeStandingOriginBlocked,
    WallAnchorOutOfBounds,
    WallAnchorNotCompletedWall,
    WallMountedOriginBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildOutcome {
    pub snapshot: FieldSnapshot,
    pub input_checksum: Sha256Digest,
    pub diagnostics: Vec<EmitterDiagnostic>,
}

impl RebuildOutcome {
    pub fn input_checksum_hex(&self) -> String {
        digest_hex(&self.input_checksum)
    }
}

pub fn rebuild_field(
    previous: Option<&FieldSnapshot>,
    input: &LightFieldInput,
) -> Result<RebuildOutcome, LightFieldError> {
    let sorted_emitters = sorted_emitters(input.emitters())?;
    let input_checksum = checksum_input(input, &sorted_emitters);
    let area = input.dimensions.area();
    let mut red = vec![0_u32; area];
    let mut green = vec![0_u32; area];
    let mut blue = vec![0_u32; area];
    let mut diagnostics = Vec::new();

    for emitter in sorted_emitters {
        if let Some(reason) = validate_emitter(emitter, input) {
            diagnostics.push(EmitterDiagnostic {
                stable_key: emitter.stable_key,
                reason,
            });
            continue;
        }
        accumulate_emitter(emitter, input, &mut red, &mut green, &mut blue);
    }

    let cells = (0..area)
        .map(|index| {
            if input.indoor_mask.as_bytes()[index] == 0 {
                return LightCell::default();
            }
            let r = red[index].min(u32::from(u16::MAX)) as u16;
            let g = green[index].min(u32::from(u16::MAX)) as u16;
            let b = blue[index].min(u32::from(u16::MAX)) as u16;
            LightCell {
                r,
                g,
                b,
                luminance: luminance(r, g, b),
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let indoor_mask = input.indoor_mask.as_bytes().to_vec().into_boxed_slice();
    let radiance_checksum = checksum_radiance(&cells);
    let mask_checksum = sha256(&indoor_mask);
    let field_checksum = checksum_field(input.dimensions, &cells, &indoor_mask);
    let (changed_radiance_cells, changed_mask_cells, changed_cell_count) =
        change_counts(previous, input.dimensions, &cells, &indoor_mask);
    let public_bytes_changed =
        previous.is_none_or(|previous| previous.field_checksum != field_checksum);
    let field_revision = match previous {
        None => 1,
        Some(previous) if public_bytes_changed => previous
            .field_revision
            .checked_add(1)
            .ok_or(LightFieldError::RevisionOverflow)?,
        Some(previous) => previous.field_revision,
    };

    Ok(RebuildOutcome {
        snapshot: FieldSnapshot {
            dimensions: input.dimensions,
            cells,
            indoor_mask,
            field_revision,
            radiance_checksum,
            mask_checksum,
            field_checksum,
            changed_radiance_cells,
            changed_mask_cells,
            changed_cell_count,
        },
        input_checksum,
        diagnostics,
    })
}

pub fn digest_hex(digest: &Sha256Digest) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn sorted_emitters(
    emitters: &[RadialLightEmitterSnapshot],
) -> Result<Vec<&RadialLightEmitterSnapshot>, LightFieldError> {
    let mut sorted = emitters.iter().collect::<Vec<_>>();
    sorted.sort_unstable_by_key(|emitter| emitter.stable_key);
    if let Some(duplicate) = sorted
        .windows(2)
        .find(|pair| pair[0].stable_key == pair[1].stable_key)
    {
        return Err(LightFieldError::DuplicateEmitterKey(
            duplicate[0].stable_key,
        ));
    }
    Ok(sorted)
}

fn validate_emitter(
    emitter: &RadialLightEmitterSnapshot,
    input: &LightFieldInput,
) -> Option<EmitterDiagnosticReason> {
    if emitter.radius_tiles.get() == 0 {
        return Some(EmitterDiagnosticReason::ZeroRadius);
    }
    match emitter.mount {
        FixtureMount::FreeStanding { origin } => {
            if !input.dimensions.contains(origin) {
                return Some(EmitterDiagnosticReason::OriginOutOfBounds);
            }
            input
                .occlusion
                .blocks_los_or_oob(origin)
                .then_some(EmitterDiagnosticReason::FreeStandingOriginBlocked)
        }
        FixtureMount::WallMounted { anchor, .. } => {
            let Some(anchor_cell) = input.occlusion.cell(anchor) else {
                return Some(EmitterDiagnosticReason::WallAnchorOutOfBounds);
            };
            if !anchor_cell.is_wall_mount_anchor() {
                return Some(EmitterDiagnosticReason::WallAnchorNotCompletedWall);
            }
            let origin = emitter.mount.origin();
            if !input.dimensions.contains(origin) {
                return Some(EmitterDiagnosticReason::OriginOutOfBounds);
            }
            input
                .occlusion
                .blocks_los_or_oob(origin)
                .then_some(EmitterDiagnosticReason::WallMountedOriginBlocked)
        }
    }
}

fn accumulate_emitter(
    emitter: &RadialLightEmitterSnapshot,
    input: &LightFieldInput,
    red: &mut [u32],
    green: &mut [u32],
    blue: &mut [u32],
) {
    let origin = emitter.mount.origin();
    let radius = i32::from(emitter.radius_tiles.get());
    let min_x = (origin.x - radius).max(0);
    let min_y = (origin.y - radius).max(0);
    let max_x = (origin.x + radius).min(i32::from(input.dimensions.width()) - 1);
    let max_y = (origin.y + radius).min(i32::from(input.dimensions.height()) - 1);
    let radius_q16 = u64::from(emitter.radius_tiles.get()) << 16;
    let base_r = mul_unorm16(emitter.color.r, emitter.intensity);
    let base_g = mul_unorm16(emitter.color.g, emitter.intensity);
    let base_b = mul_unorm16(emitter.color.b, emitter.intensity);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let target = LightGridPos::new(x, y);
            let index = input
                .dimensions
                .index(target)
                .expect("bounded field accumulation target");
            if input.indoor_mask.as_bytes()[index] == 0 {
                continue;
            }
            let delta_x = i64::from(x - origin.x);
            let delta_y = i64::from(y - origin.y);
            let squared = u128::from((delta_x * delta_x + delta_y * delta_y) as u64);
            let distance_q16 = (squared << 32).isqrt() as u64;
            if distance_q16 >= radius_q16 || !has_line_of_sight(origin, target, &input.occlusion) {
                continue;
            }
            let falloff = round_ratio_to_unorm16(radius_q16 - distance_q16, radius_q16);
            red[index] = red[index].saturating_add(u32::from(mul_unorm16(base_r, falloff)));
            green[index] = green[index].saturating_add(u32::from(mul_unorm16(base_g, falloff)));
            blue[index] = blue[index].saturating_add(u32::from(mul_unorm16(base_b, falloff)));
        }
    }
}

fn mul_unorm16(left: u16, right: u16) -> u16 {
    let product = u64::from(left) * u64::from(right);
    ((product + 32_767) / 65_535) as u16
}

fn round_ratio_to_unorm16(numerator: u64, denominator: u64) -> u16 {
    ((u128::from(numerator) * u128::from(u16::MAX) + u128::from(denominator / 2))
        / u128::from(denominator)) as u16
}

fn luminance(r: u16, g: u16, b: u16) -> u16 {
    ((13_933_u64 * u64::from(r) + 46_871_u64 * u64::from(g) + 4_732_u64 * u64::from(b) + 32_768)
        >> 16) as u16
}

fn checksum_input(
    input: &LightFieldInput,
    emitters: &[&RadialLightEmitterSnapshot],
) -> Sha256Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"P03-input-v1");
    hasher.update(input.dimensions.width().to_le_bytes());
    hasher.update(input.dimensions.height().to_le_bytes());
    hasher.update(input.indoor_mask.as_bytes());
    for cell in input.occlusion.as_cells() {
        hasher.update([cell.canonical_byte()]);
    }
    for emitter in emitters {
        hasher.update(emitter.stable_key.to_le_bytes());
        let (mount_kind, origin, anchor, inward) = match emitter.mount {
            FixtureMount::FreeStanding { origin } => {
                (0_u8, origin, LightGridPos::new(0, 0), (0_i8, 0_i8))
            }
            FixtureMount::WallMounted { anchor, inward } => {
                let delta = inward.delta();
                (
                    1_u8,
                    emitter.mount.origin(),
                    anchor,
                    (delta.0 as i8, delta.1 as i8),
                )
            }
        };
        hasher.update([mount_kind]);
        hasher.update(origin.x.to_le_bytes());
        hasher.update(origin.y.to_le_bytes());
        hasher.update(anchor.x.to_le_bytes());
        hasher.update(anchor.y.to_le_bytes());
        hasher.update([inward.0 as u8, inward.1 as u8]);
        hasher.update(emitter.radius_tiles.get().to_le_bytes());
        hasher.update(emitter.color.r.to_le_bytes());
        hasher.update(emitter.color.g.to_le_bytes());
        hasher.update(emitter.color.b.to_le_bytes());
        hasher.update(emitter.intensity.to_le_bytes());
    }
    hasher.finalize().into()
}

fn checksum_radiance(cells: &[LightCell]) -> Sha256Digest {
    let mut hasher = Sha256::new();
    update_radiance(&mut hasher, cells);
    hasher.finalize().into()
}

fn checksum_field(dimensions: GridDimensions, cells: &[LightCell], mask: &[u8]) -> Sha256Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"P03-field-v1");
    hasher.update(dimensions.width().to_le_bytes());
    hasher.update(dimensions.height().to_le_bytes());
    update_radiance(&mut hasher, cells);
    hasher.update(mask);
    hasher.finalize().into()
}

fn update_radiance(hasher: &mut Sha256, cells: &[LightCell]) {
    for cell in cells {
        hasher.update(cell.r.to_le_bytes());
        hasher.update(cell.g.to_le_bytes());
        hasher.update(cell.b.to_le_bytes());
        hasher.update(cell.luminance.to_le_bytes());
    }
}

fn sha256(bytes: &[u8]) -> Sha256Digest {
    Sha256::digest(bytes).into()
}

fn change_counts(
    previous: Option<&FieldSnapshot>,
    dimensions: GridDimensions,
    cells: &[LightCell],
    mask: &[u8],
) -> (u32, u32, u32) {
    let comparable = previous.filter(|previous| previous.dimensions == dimensions);
    let mut radiance = 0_u32;
    let mut mask_changes = 0_u32;
    let mut either = 0_u32;
    for index in 0..cells.len() {
        let previous_cell = comparable
            .and_then(|previous| previous.cells.get(index))
            .copied()
            .unwrap_or_default();
        let previous_mask = comparable
            .and_then(|previous| previous.indoor_mask.get(index))
            .copied()
            .unwrap_or_default();
        let radiance_changed = previous_cell != cells[index];
        let mask_changed = previous_mask != mask[index];
        radiance += u32::from(radiance_changed);
        mask_changes += u32::from(mask_changed);
        either += u32::from(radiance_changed || mask_changed);
    }
    (radiance, mask_changes, either)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::{
        CardinalDirection, LightRadiusTiles, LightRgbLinear, OcclusionCell, pack_rgba8_linear,
    };

    fn input(
        dimensions: GridDimensions,
        mask: IndoorMask,
        cells: Vec<OcclusionCell>,
        emitters: Vec<RadialLightEmitterSnapshot>,
    ) -> LightFieldInput {
        LightFieldInput::new(
            dimensions,
            mask,
            LightOcclusionGrid::from_cells(dimensions, cells).unwrap(),
            emitters,
        )
        .unwrap()
    }

    fn emitter(stable_key: u64, origin: LightGridPos) -> RadialLightEmitterSnapshot {
        RadialLightEmitterSnapshot {
            stable_key,
            mount: FixtureMount::FreeStanding { origin },
            radius_tiles: LightRadiusTiles::new(5),
            color: LightRgbLinear::WHITE,
            intensity: u16::MAX,
        }
    }

    #[test]
    fn fixed_vector_uses_q16_falloff_luminance_and_little_endian_payload() {
        let dimensions = GridDimensions::new(5, 1).unwrap();
        let outcome = rebuild_field(
            None,
            &input(
                dimensions,
                IndoorMask::filled(dimensions, true),
                vec![OcclusionCell::Clear; dimensions.area()],
                vec![emitter(1, LightGridPos::new(0, 0))],
            ),
        )
        .unwrap();
        assert_eq!(
            outcome.snapshot.cells()[0],
            LightCell {
                r: 65_535,
                g: 65_535,
                b: 65_535,
                luminance: 65_535,
            }
        );
        assert_eq!(outcome.snapshot.cells()[1].r, 52_428);
        assert_eq!(outcome.snapshot.logical_payload_bytes(), 40);
        assert_eq!(
            &outcome.snapshot.radiance_payload_le()[..8],
            &[255, 255, 255, 255, 255, 255, 255, 255]
        );
        assert_eq!(pack_rgba8_linear(&outcome.snapshot).len(), 20);
    }

    #[test]
    fn emitter_order_does_not_change_bytes_or_checksums() {
        let dimensions = GridDimensions::new(8, 8).unwrap();
        let emitters = vec![
            emitter(9, LightGridPos::new(2, 2)),
            emitter(3, LightGridPos::new(5, 5)),
            emitter(6, LightGridPos::new(2, 5)),
        ];
        let make_input = |emitters| {
            input(
                dimensions,
                IndoorMask::filled(dimensions, true),
                vec![OcclusionCell::Clear; dimensions.area()],
                emitters,
            )
        };
        let forward = rebuild_field(None, &make_input(emitters.clone())).unwrap();
        let reverse =
            rebuild_field(None, &make_input(emitters.into_iter().rev().collect())).unwrap();
        assert_eq!(forward.input_checksum, reverse.input_checksum);
        assert_eq!(forward.snapshot.cells, reverse.snapshot.cells);
        assert_eq!(
            forward.snapshot.field_checksum,
            reverse.snapshot.field_checksum
        );
    }

    #[test]
    fn duplicate_keys_are_rejected_before_rebuild() {
        let dimensions = GridDimensions::new(2, 2).unwrap();
        let duplicate = emitter(1, LightGridPos::new(0, 0));
        let result = rebuild_field(
            None,
            &input(
                dimensions,
                IndoorMask::filled(dimensions, true),
                vec![OcclusionCell::Clear; dimensions.area()],
                vec![duplicate, duplicate],
            ),
        );
        assert!(matches!(
            result,
            Err(LightFieldError::DuplicateEmitterKey(1))
        ));
    }

    #[test]
    fn invalid_emitters_fail_dark_with_stable_diagnostics() {
        let dimensions = GridDimensions::new(3, 3).unwrap();
        let mut cells = vec![OcclusionCell::Clear; dimensions.area()];
        cells[dimensions.index(LightGridPos::new(0, 0)).unwrap()] = OcclusionCell::CompletedWall;
        let emitters = vec![
            RadialLightEmitterSnapshot {
                radius_tiles: LightRadiusTiles::new(0),
                ..emitter(7, LightGridPos::new(1, 1))
            },
            emitter(2, LightGridPos::new(0, 0)),
            RadialLightEmitterSnapshot {
                stable_key: 5,
                mount: FixtureMount::WallMounted {
                    anchor: LightGridPos::new(1, 1),
                    inward: CardinalDirection::North,
                },
                radius_tiles: LightRadiusTiles::new(5),
                color: LightRgbLinear::WHITE,
                intensity: u16::MAX,
            },
        ];
        let outcome = rebuild_field(
            None,
            &input(
                dimensions,
                IndoorMask::filled(dimensions, true),
                cells,
                emitters,
            ),
        )
        .unwrap();
        assert_eq!(
            outcome.diagnostics,
            vec![
                EmitterDiagnostic {
                    stable_key: 2,
                    reason: EmitterDiagnosticReason::FreeStandingOriginBlocked,
                },
                EmitterDiagnostic {
                    stable_key: 5,
                    reason: EmitterDiagnosticReason::WallAnchorNotCompletedWall,
                },
                EmitterDiagnostic {
                    stable_key: 7,
                    reason: EmitterDiagnosticReason::ZeroRadius,
                },
            ]
        );
        assert!(
            outcome
                .snapshot
                .cells()
                .iter()
                .all(|cell| *cell == LightCell::default())
        );
    }

    #[test]
    fn outside_mask_source_can_light_inside_but_outside_targets_stay_dark() {
        let dimensions = GridDimensions::new(4, 1).unwrap();
        let mask = IndoorMask::from_bytes(dimensions, vec![0, 0, 1, 1]).unwrap();
        let outcome = rebuild_field(
            None,
            &input(
                dimensions,
                mask,
                vec![OcclusionCell::Clear; dimensions.area()],
                vec![emitter(1, LightGridPos::new(0, 0))],
            ),
        )
        .unwrap();
        assert_eq!(outcome.snapshot.cells()[0], LightCell::default());
        assert!(outcome.snapshot.cells()[2].r > 0);
        assert_eq!(
            outcome.snapshot.sample_luminance(LightGridPos::new(0, 0)),
            Some(0)
        );
        assert_eq!(
            outcome.snapshot.sample_room_cell(LightGridPos::new(0, 0)),
            Err(FieldSampleError::OutsideIndoorMask(LightGridPos::new(0, 0)))
        );
        assert_eq!(
            outcome.snapshot.sample_luminance(LightGridPos::new(-1, 0)),
            None
        );
        assert_eq!(
            outcome.snapshot.sample_room_cell(LightGridPos::new(-1, 0)),
            Err(FieldSampleError::OutOfBounds(LightGridPos::new(-1, 0)))
        );
    }

    #[test]
    fn revision_tracks_radiance_and_mask_public_bytes_only() {
        let dimensions = GridDimensions::new(2, 1).unwrap();
        let clear = vec![OcclusionCell::Clear; dimensions.area()];
        let first_input = input(
            dimensions,
            IndoorMask::from_bytes(dimensions, vec![1, 0]).unwrap(),
            clear.clone(),
            vec![],
        );
        let first = rebuild_field(None, &first_input).unwrap();
        assert_eq!(first.snapshot.field_revision(), 1);
        assert_eq!(first.snapshot.changed_mask_cells(), 1);

        let same = rebuild_field(Some(&first.snapshot), &first_input).unwrap();
        assert_eq!(same.snapshot.field_revision(), 1);
        assert_eq!(same.snapshot.changed_cell_count(), 0);

        let mask_changed = rebuild_field(
            Some(&same.snapshot),
            &input(
                dimensions,
                IndoorMask::from_bytes(dimensions, vec![1, 1]).unwrap(),
                clear,
                vec![],
            ),
        )
        .unwrap();
        assert_eq!(mask_changed.snapshot.field_revision(), 2);
        assert_eq!(mask_changed.snapshot.changed_radiance_cells(), 0);
        assert_eq!(mask_changed.snapshot.changed_mask_cells(), 1);
        assert_eq!(mask_changed.snapshot.changed_cell_count(), 1);

        let radiance_changed = rebuild_field(
            Some(&mask_changed.snapshot),
            &input(
                dimensions,
                IndoorMask::from_bytes(dimensions, vec![1, 1]).unwrap(),
                vec![OcclusionCell::Clear; dimensions.area()],
                vec![emitter(1, LightGridPos::new(0, 0))],
            ),
        )
        .unwrap();
        assert_eq!(radiance_changed.snapshot.field_revision(), 3);
        assert_eq!(radiance_changed.snapshot.changed_mask_cells(), 0);
        assert_eq!(radiance_changed.snapshot.changed_radiance_cells(), 2);

        let both_changed = rebuild_field(
            Some(&radiance_changed.snapshot),
            &input(
                dimensions,
                IndoorMask::from_bytes(dimensions, vec![1, 0]).unwrap(),
                vec![OcclusionCell::Clear; dimensions.area()],
                vec![],
            ),
        )
        .unwrap();
        assert_eq!(both_changed.snapshot.field_revision(), 4);
        assert_eq!(both_changed.snapshot.changed_mask_cells(), 1);
        assert_eq!(both_changed.snapshot.changed_radiance_cells(), 2);

        let mut exhausted = both_changed.snapshot;
        exhausted.field_revision = u64::MAX;
        let overflow = rebuild_field(
            Some(&exhausted),
            &input(
                dimensions,
                IndoorMask::from_bytes(dimensions, vec![0, 0]).unwrap(),
                vec![OcclusionCell::Clear; dimensions.area()],
                vec![],
            ),
        );
        assert!(matches!(overflow, Err(LightFieldError::RevisionOverflow)));
    }
}
