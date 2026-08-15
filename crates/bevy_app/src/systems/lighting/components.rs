use bevy::prelude::*;
use hw_infra::lighting::{FixtureMount, LightGridPos, LightRadiusTiles, LightRgbLinear};

/// Runtime-only marker and configuration for a fixture that contributes to
/// the shared indoor CPU Light Field.
///
/// P05 owns persistence registration. P04 keeps the component reconstructible
/// from the completed building root and uses P03's pure [`FixtureMount`] value.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadialLightEmitter {
    pub mount: FixtureMount,
    pub radius_tiles: LightRadiusTiles,
    pub color: LightRgbLinear,
    pub intensity: u16,
}

impl RadialLightEmitter {
    pub const fn outdoor_lamp(grid: (i32, i32)) -> Self {
        Self {
            mount: FixtureMount::FreeStanding {
                origin: LightGridPos::new(grid.0, grid.1),
            },
            radius_tiles: LightRadiusTiles::new(5),
            color: LightRgbLinear {
                r: u16::MAX,
                g: 49_151,
                b: 32_767,
            },
            intensity: u16::MAX,
        }
    }
}
