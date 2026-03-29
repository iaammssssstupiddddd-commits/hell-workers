mod observers;
mod sync;

pub use observers::*;
pub use sync::*;

use hw_core::visual_mirror::building::BuildingTypeVisual;

use crate::model::BuildingType;

fn building_type_to_visual(kind: BuildingType) -> BuildingTypeVisual {
    match kind {
        BuildingType::Wall => BuildingTypeVisual::Wall,
        BuildingType::Door => BuildingTypeVisual::Door,
        BuildingType::Floor => BuildingTypeVisual::Floor,
        BuildingType::Tank => BuildingTypeVisual::Tank,
        BuildingType::MudMixer => BuildingTypeVisual::MudMixer,
        BuildingType::RestArea => BuildingTypeVisual::RestArea,
        BuildingType::Bridge => BuildingTypeVisual::Bridge,
        BuildingType::SandPile => BuildingTypeVisual::SandPile,
        BuildingType::BonePile => BuildingTypeVisual::BonePile,
        BuildingType::WheelbarrowParking => BuildingTypeVisual::WheelbarrowParking,
        BuildingType::SoulSpa => BuildingTypeVisual::SoulSpa,
        BuildingType::OutdoorLamp => BuildingTypeVisual::OutdoorLamp,
    }
}
