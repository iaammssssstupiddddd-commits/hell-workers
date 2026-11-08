//! Stable identities and legal-space contract, independent of allocator Entity IDs.

use bevy::prelude::*;
use hw_jobs::BuildingType;
use hw_world::{Site, WorldMap, Yard, layout::RIVER_Y_MIN};

use super::super::PerfScenarioSize;
use crate::interface::selection::placement_geometry::building_geometry;

pub(super) const CAMERA_SCALE: f32 = 5.0;

pub(super) fn requires_visible(
    session: Option<&crate::assets::building_asset_set::acceptance::BuildingArtSession>,
    candidate: bool,
    ordinal: usize,
) -> bool {
    use crate::assets::building_asset_set::acceptance::BuildingArtView;
    session.is_none_or(|s| s.view == BuildingArtView::Overview) || (candidate && ordinal < 4)
}

pub(super) fn camera_frame(
    session: Option<&crate::assets::building_asset_set::acceptance::BuildingArtSession>,
    size: PerfScenarioSize,
) -> (Vec2, f32) {
    use crate::assets::building_asset_set::{BuildingAssetKind, acceptance::BuildingArtView};
    let Some(session) = session.filter(|s| s.view != BuildingArtView::Overview) else {
        return (WorldMap::grid_to_world(46, 37), CAMERA_SCALE);
    };
    let kind = match session.identity.kind {
        BuildingAssetKind::Tank => BuildingType::Tank,
        BuildingAssetKind::MudMixer => BuildingType::MudMixer,
        BuildingAssetKind::RestArea => BuildingType::RestArea,
        BuildingAssetKind::SoulSpa => BuildingType::SoulSpa,
        BuildingAssetKind::WheelbarrowParking => BuildingType::WheelbarrowParking,
        BuildingAssetKind::SandPile => BuildingType::SandPile,
        BuildingAssetKind::BonePile => BuildingType::BonePile,
        BuildingAssetKind::OutdoorLamp => BuildingType::OutdoorLamp,
        BuildingAssetKind::Bridge => unreachable!("Bridge is rejected at art-session admission"),
    };
    let center = layout(size)
        .iter()
        .filter(|s| s.kind == kind && s.ordinal < 4)
        .map(|s| s.center)
        .sum::<Vec2>()
        / 4.0;
    (
        if session.is_context() {
            center - Vec2::Y * 80.0
        } else {
            center
        },
        session.view.scale(),
    )
}
// Bridge placement/worldgen is a separate issue; it must not gate this reference.
pub(super) const KINDS: [BuildingType; 9] = [
    BuildingType::Tank,
    BuildingType::MudMixer,
    BuildingType::RestArea,
    BuildingType::SoulSpa,
    BuildingType::WheelbarrowParking,
    BuildingType::SandPile,
    BuildingType::BonePile,
    BuildingType::Door,
    BuildingType::OutdoorLamp,
];

#[derive(Clone, Debug)]
pub(super) struct Specimen {
    pub ordinal: usize,
    pub kind: BuildingType,
    pub anchor: (i32, i32),
    pub tiles: Vec<(i32, i32)>,
    pub center: Vec2,
    pub spa_mask_override: Option<u8>,
}

impl Specimen {
    pub fn quarter(&self) -> usize {
        self.ordinal % 4
    }

    pub fn water_count(&self) -> usize {
        [0, 25, 50, 50][self.quarter()]
    }

    pub fn rest_count(&self) -> usize {
        [0, 1, hw_core::constants::REST_AREA_CAPACITY, 0][self.quarter()]
    }

    pub fn spa_mask(&self) -> u8 {
        self.spa_mask_override
            .unwrap_or([0, 1, 3, 15][self.quarter()])
    }

    pub fn supports(&self) -> [(i32, i32); 2] {
        [
            (self.anchor.0 - 1, self.anchor.1),
            (self.anchor.0 + 1, self.anchor.1),
        ]
    }

    pub fn companion(&self) -> (i32, i32) {
        (self.anchor.0, self.anchor.1 + 2)
    }
}

pub(super) fn copies(size: PerfScenarioSize) -> usize {
    match size {
        PerfScenarioSize::Small => 4,
        PerfScenarioSize::Medium => 16,
        PerfScenarioSize::Large => unreachable!("building-art-static rejects large before startup"),
    }
}

pub(super) fn apply_spa_mask_page(specs: &mut [Specimen], page: u8) -> Result<(), String> {
    let spa = specs
        .iter()
        .filter(|spec| spec.kind == BuildingType::SoulSpa)
        .collect::<Vec<_>>();
    if page > 3
        || spa.len() != 4
        || spa
            .iter()
            .enumerate()
            .any(|(index, spec)| spec.ordinal != index)
    {
        return Err("Spa page requires exactly four ordered specimens and page 0..3".into());
    }
    for spec in specs
        .iter_mut()
        .filter(|spec| spec.kind == BuildingType::SoulSpa)
    {
        spec.spa_mask_override = Some(page * 4 + spec.ordinal as u8);
    }
    Ok(())
}

pub(super) fn layout(size: PerfScenarioSize) -> Vec<Specimen> {
    KINDS
        .into_iter()
        .enumerate()
        .flat_map(|(row, kind)| {
            (0..copies(size)).map(move |ordinal| {
                let x = 8 + i32::try_from(ordinal).expect("bounded column") * 5;
                let anchor = (x, 8 + i32::try_from(row).expect("bounded row") * 5);
                let geometry = building_geometry(kind, anchor, RIVER_Y_MIN);
                Specimen {
                    ordinal,
                    kind,
                    anchor,
                    tiles: geometry.occupied_grids,
                    center: geometry.draw_pos,
                    spa_mask_override: None,
                }
            })
        })
        .collect()
}

pub(super) fn site() -> Site {
    Site {
        min: WorldMap::grid_to_world(5, 5),
        max: WorldMap::grid_to_world(88, 72),
    }
}

/// Every column has a separate grid. The first two lamps have their own unpowered
/// yard, so a one-worker Spa in column 1 cannot accidentally power its off control.
pub(super) fn yards(size: PerfScenarioSize) -> Vec<Yard> {
    (0..copies(size))
        .flat_map(|ordinal| {
            let x = 8 + i32::try_from(ordinal).expect("bounded column") * 5;
            let bottom = if ordinal % 4 < 2 { 45 } else { 50 };
            let mut yards = vec![Yard {
                min: WorldMap::grid_to_world(x - 1, 5),
                max: WorldMap::grid_to_world(x + 2, bottom),
            }];
            if ordinal % 4 < 2 {
                yards.push(Yard {
                    min: WorldMap::grid_to_world(x - 1, 47),
                    max: WorldMap::grid_to_world(x + 2, 50),
                });
            }
            yards
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn explicit_spa_pages_cover_all_masks_without_changing_other_specimens() {
        let mut masks = Vec::new();
        for (page, actors) in [12, 16, 16, 20].into_iter().enumerate() {
            let mut specs = layout(PerfScenarioSize::Small);
            apply_spa_mask_page(&mut specs, page as u8).unwrap();
            let spa = specs
                .iter()
                .filter(|s| s.kind == BuildingType::SoulSpa)
                .collect::<Vec<_>>();
            let page_masks = spa.iter().map(|s| s.spa_mask()).collect::<Vec<_>>();
            assert_eq!(
                8 + page_masks.iter().map(|m| m.count_ones()).sum::<u32>(),
                actors
            );
            masks.extend(page_masks);
            assert!(
                specs
                    .iter()
                    .filter(|s| s.kind != BuildingType::SoulSpa)
                    .all(|s| s.spa_mask_override.is_none())
            );
        }
        assert_eq!(masks, (0..16).collect::<Vec<_>>());
        let mut small = layout(PerfScenarioSize::Small);
        assert!(apply_spa_mask_page(&mut small, 4).is_err());
        assert!(small.iter().all(|s| s.spa_mask_override.is_none()));
        assert!(apply_spa_mask_page(&mut layout(PerfScenarioSize::Medium), 0).is_err());
    }

    #[test]
    fn candidate_camera_frames_only_its_first_four_and_preserves_overview() {
        use crate::assets::building_asset_set::{
            BuildingAssetAuthority, BuildingAssetKind, BuildingAssetSetIdentity,
            acceptance::{BuildingArtSession, BuildingArtView},
        };
        let mut session = BuildingArtSession {
            mode: "feedback".into(),
            identity: BuildingAssetSetIdentity {
                kind: BuildingAssetKind::RestArea,
                generation: 1,
                authority: BuildingAssetAuthority::ArtPreview,
                manifest_sha256: "a".repeat(64),
            },
            locator: "manifests/building-rest-area-v1.buildingset".into(),
            status_path: "/unused/probe.json".into(),
            nonce: "a".repeat(32),
            view: BuildingArtView::Near,
            variant: Default::default(),
            lighting: Default::default(),
            spa_mask_page: None,
        };
        let expected = layout(PerfScenarioSize::Small)
            .iter()
            .filter(|s| s.kind == BuildingType::RestArea)
            .map(|s| s.center)
            .sum::<Vec2>()
            / 4.0;
        for size in [PerfScenarioSize::Small, PerfScenarioSize::Medium] {
            assert_eq!(camera_frame(Some(&session), size), (expected, 0.5));
        }
        assert!(requires_visible(Some(&session), true, 0));
        assert!(!requires_visible(Some(&session), true, 4));
        assert!(!requires_visible(Some(&session), false, 0));
        assert!(requires_visible(None, false, 0));
        session.view = BuildingArtView::Normal;
        assert_eq!(
            camera_frame(Some(&session), PerfScenarioSize::Small),
            (expected, 1.0)
        );
        session.view = BuildingArtView::Far;
        assert_eq!(
            camera_frame(Some(&session), PerfScenarioSize::Small),
            (expected, 2.0)
        );
        session.view = BuildingArtView::Overview;
        assert!(requires_visible(Some(&session), false, 0));
        assert_eq!(
            camera_frame(Some(&session), PerfScenarioSize::Small),
            camera_frame(None, PerfScenarioSize::Small)
        );
    }

    #[test]
    fn exact_inventory_and_disjoint_footprints_include_supports_and_companions() {
        for size in [PerfScenarioSize::Small, PerfScenarioSize::Medium] {
            let specs = layout(size);
            assert_eq!(specs.len(), copies(size) * 9);
            assert!(!KINDS.contains(&BuildingType::Bridge));
            let mut cells = HashSet::new();
            for spec in &specs {
                for &(x, y) in &spec.tiles {
                    assert!((0..100).contains(&x) && (0..100).contains(&y));
                    assert!(cells.insert((x, y)), "overlap: {spec:?}");
                }
                if spec.kind == BuildingType::Tank {
                    let (x, y) = spec.companion();
                    assert!(cells.insert((x, y)) && cells.insert((x + 1, y)));
                }
                if spec.kind == BuildingType::Door {
                    for grid in spec.supports() {
                        assert!(cells.insert(grid));
                    }
                }
            }
            for kind in KINDS {
                assert_eq!(
                    specs.iter().filter(|s| s.kind == kind).count(),
                    copies(size)
                );
            }
            let actors: usize = specs
                .iter()
                .map(|s| match s.kind {
                    BuildingType::MudMixer => usize::from(s.quarter() >= 2),
                    BuildingType::RestArea => s.rest_count(),
                    BuildingType::SoulSpa => s.spa_mask().count_ones() as usize,
                    _ => 0,
                })
                .sum();
            assert_eq!(actors, copies(size) / 4 * 15);
            let yards = yards(size);
            for s in &specs {
                assert!(site().contains(s.center));
                assert_eq!(yards.iter().filter(|y| y.contains(s.center)).count(), 1);
            }
        }
    }

    #[test]
    fn small_is_an_exact_subset_and_spa_uses_southward_anchor() {
        let medium = layout(PerfScenarioSize::Medium);
        for small in layout(PerfScenarioSize::Small) {
            let large = medium
                .iter()
                .find(|s| s.kind == small.kind && s.ordinal == small.ordinal)
                .unwrap();
            assert_eq!(small.tiles, large.tiles);
            assert_eq!(small.center, large.center);
            if small.kind == BuildingType::SoulSpa {
                assert_eq!(small.tiles[2], (small.anchor.0, small.anchor.1 - 1));
            }
        }
    }
}
