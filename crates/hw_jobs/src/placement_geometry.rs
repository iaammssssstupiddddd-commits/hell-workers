use crate::BuildingType;

pub type GridOffset = (i32, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingAnchorBasis {
    ClickedGrid,
    RiverYMin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildingShape {
    pub ordered_relative_tiles: &'static [GridOffset],
    pub center_offset_tiles: (f32, f32),
    pub size_tiles: (f32, f32),
    pub anchor_basis: BuildingAnchorBasis,
}

const SINGLE_TILE: &[GridOffset] = &[(0, 0)];
const TWO_BY_TWO: &[GridOffset] = &[(0, 0), (1, 0), (0, 1), (1, 1)];
const SOUL_SPA: &[GridOffset] = &[(0, 0), (1, 0), (0, -1), (1, -1)];
const BRIDGE: &[GridOffset] = &[
    (0, 0),
    (1, 0),
    (0, 1),
    (1, 1),
    (0, 2),
    (1, 2),
    (0, 3),
    (1, 3),
    (0, 4),
    (1, 4),
];

pub const fn building_shape(kind: BuildingType) -> BuildingShape {
    match kind {
        BuildingType::Bridge => BuildingShape {
            ordered_relative_tiles: BRIDGE,
            center_offset_tiles: (0.5, 2.0),
            size_tiles: (2.0, 5.0),
            anchor_basis: BuildingAnchorBasis::RiverYMin,
        },
        BuildingType::Tank
        | BuildingType::MudMixer
        | BuildingType::RestArea
        | BuildingType::WheelbarrowParking => BuildingShape {
            ordered_relative_tiles: TWO_BY_TWO,
            center_offset_tiles: (0.5, 0.5),
            size_tiles: (2.0, 2.0),
            anchor_basis: BuildingAnchorBasis::ClickedGrid,
        },
        BuildingType::SoulSpa => BuildingShape {
            ordered_relative_tiles: SOUL_SPA,
            center_offset_tiles: (0.5, -0.5),
            size_tiles: (2.0, 2.0),
            anchor_basis: BuildingAnchorBasis::ClickedGrid,
        },
        BuildingType::Wall
        | BuildingType::Door
        | BuildingType::Floor
        | BuildingType::SandPile
        | BuildingType::BonePile
        | BuildingType::OutdoorLamp => BuildingShape {
            ordered_relative_tiles: SINGLE_TILE,
            center_offset_tiles: (0.0, 0.0),
            size_tiles: (1.0, 1.0),
            anchor_basis: BuildingAnchorBasis::ClickedGrid,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_building_has_a_nonempty_ordered_shape() {
        for kind in BuildingType::ALL {
            let shape = building_shape(kind);
            assert!(!shape.ordered_relative_tiles.is_empty(), "{kind:?}");
            assert_eq!(shape.ordered_relative_tiles[0], (0, 0), "{kind:?}");
        }
    }

    #[test]
    fn bridge_and_soul_spa_keep_their_asymmetric_anchor_contracts() {
        let bridge = building_shape(BuildingType::Bridge);
        assert_eq!(bridge.anchor_basis, BuildingAnchorBasis::RiverYMin);
        assert_eq!(bridge.ordered_relative_tiles.len(), 10);
        assert_eq!(bridge.ordered_relative_tiles[9], (1, 4));
        assert_eq!(bridge.center_offset_tiles, (0.5, 2.0));

        let spa = building_shape(BuildingType::SoulSpa);
        assert_eq!(spa.anchor_basis, BuildingAnchorBasis::ClickedGrid);
        assert_eq!(spa.ordered_relative_tiles, SOUL_SPA);
        assert_eq!(spa.center_offset_tiles, (0.5, -0.5));
    }
}
