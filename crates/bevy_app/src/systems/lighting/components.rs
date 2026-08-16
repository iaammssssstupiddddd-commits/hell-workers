use bevy::prelude::*;
use hw_infra::lighting::{FixtureMount, LightGridPos, LightRadiusTiles, LightRgbLinear};
use serde::{Deserialize, Serialize};

/// Durable Bevy adapter for P03's pure fixture-mount value.
///
/// The wrapper is opaque to reflection so `hw_infra` stays free of Bevy
/// dependencies. Reflection serialization delegates to the pure serde value.
#[derive(Component, Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(opaque)]
#[reflect(Component, Serialize, Deserialize)]
pub struct LightingFixtureMount(pub FixtureMount);

impl LightingFixtureMount {
    pub const fn free_standing(grid: (i32, i32)) -> Self {
        Self(FixtureMount::FreeStanding {
            origin: LightGridPos::new(grid.0, grid.1),
        })
    }

    pub const fn mount(self) -> FixtureMount {
        self.0
    }
}

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
        Self::outdoor_lamp_at_mount(LightingFixtureMount::free_standing(grid).mount())
    }

    pub const fn outdoor_lamp_at_mount(mount: FixtureMount) -> Self {
        Self {
            mount,
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
