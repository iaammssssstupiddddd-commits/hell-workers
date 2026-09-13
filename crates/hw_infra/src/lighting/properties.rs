use super::*;
use proptest::prelude::*;

fn channel() -> impl Strategy<Value = u16> {
    prop_oneof![Just(0), Just(u16::MAX), any::<u16>()]
}

fn light_input() -> impl Strategy<Value = LightFieldInput> {
    (1_u16..=8, 1_u16..=8).prop_flat_map(|(width, height)| {
        let dimensions = GridDimensions::new(width, height).unwrap();
        let emitter = (
            -1_i32..=i32::from(width),
            -1_i32..=i32::from(height),
            0_u16..=8,
            channel(),
            channel(),
            channel(),
            channel(),
        );
        (
            prop::collection::vec(0_u8..=1, dimensions.area()),
            prop::collection::vec(0_usize..6, dimensions.area()),
            prop::collection::vec(emitter, 0..=6),
        )
            .prop_map(move |(mask, cells, emitters)| {
                let kinds = [
                    OcclusionCell::Clear,
                    OcclusionCell::ProvisionalWall,
                    OcclusionCell::OpenDoor,
                    OcclusionCell::CompletedWall,
                    OcclusionCell::ClosedDoor,
                    OcclusionCell::LockedDoor,
                ];
                LightFieldInput::new(
                    dimensions,
                    IndoorMask::from_bytes(dimensions, mask).unwrap(),
                    LightOcclusionGrid::from_cells(
                        dimensions,
                        cells.into_iter().map(|i| kinds[i]).collect(),
                    )
                    .unwrap(),
                    emitters
                        .into_iter()
                        .enumerate()
                        .map(|(key, (x, y, radius, r, g, b, intensity))| {
                            RadialLightEmitterSnapshot {
                                stable_key: key as u64,
                                mount: FixtureMount::FreeStanding {
                                    origin: LightGridPos::new(x, y),
                                },
                                radius_tiles: LightRadiusTiles::new(radius),
                                color: LightRgbLinear { r, g, b },
                                intensity,
                            }
                        })
                        .collect(),
                )
                .unwrap()
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, max_shrink_iters: 1024, ..ProptestConfig::default() })]

    #[test]
    fn properties_emitter_permutation_preserves_field(
        input in light_input(), priorities in prop::collection::vec(any::<u32>(), 6),
    ) {
        let mut emitters = input.emitters().to_vec();
        emitters.sort_by_key(|emitter| (priorities[emitter.stable_key as usize], emitter.stable_key));
        let permuted = LightFieldInput::new(
            input.dimensions(), input.indoor_mask().clone(), input.occlusion().clone(), emitters,
        ).unwrap();
        // Includes canonical input, every cell/checksum, revision and diagnostics.
        prop_assert_eq!(rebuild_field(None, &input).unwrap(), rebuild_field(None, &permuted).unwrap());
    }

    #[test]
    fn properties_reapplying_input_has_no_changes(input in light_input()) {
        let first = rebuild_field(None, &input).unwrap();
        let repeated = rebuild_field(Some(&first.snapshot), &input).unwrap();
        let before = &first.snapshot;
        let after = &repeated.snapshot;
        prop_assert_eq!(before.cells(), after.cells());
        prop_assert_eq!(before.indoor_mask_bytes(), after.indoor_mask_bytes());
        prop_assert_eq!(before.field_revision(), after.field_revision());
        prop_assert_eq!(before.radiance_checksum(), after.radiance_checksum());
        prop_assert_eq!(before.mask_checksum(), after.mask_checksum());
        prop_assert_eq!(before.field_checksum(), after.field_checksum());
        prop_assert_eq!(first.input_checksum, repeated.input_checksum);
        prop_assert_eq!(first.diagnostics, repeated.diagnostics);
        prop_assert_eq!(after.changed_radiance_cells(), 0);
        prop_assert_eq!(after.changed_mask_cells(), 0);
        prop_assert_eq!(after.changed_cell_count(), 0);
    }
}
