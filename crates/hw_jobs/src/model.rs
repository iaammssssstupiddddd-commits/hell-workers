use std::collections::HashMap;

use bevy::prelude::*;

pub use hw_core::jobs::WorkType;
use hw_core::logistics::ResourceType;
pub use hw_core::relationships::ManagedBy as IssuedBy;
pub use hw_core::world::DoorState;
use hw_core::constants::DOOR_CLOSE_DELAY_SECS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum BuildingType {
    #[default]
    Wall,
    Door,
    Floor,
    Tank,
    MudMixer,
    RestArea,
    Bridge,
    SandPile,
    BonePile,
    WheelbarrowParking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCategory {
    Structure,
    Architecture,
    Plant,
    Temporary,
}

impl BuildingCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Architecture => "Architecture",
            Self::Plant => "Plant",
            Self::Temporary => "Temporary",
        }
    }
}

impl BuildingType {
    pub fn category(&self) -> BuildingCategory {
        match self {
            BuildingType::Wall | BuildingType::Floor | BuildingType::Bridge => {
                BuildingCategory::Structure
            }
            BuildingType::Door => BuildingCategory::Architecture,
            BuildingType::Tank | BuildingType::MudMixer => BuildingCategory::Plant,
            BuildingType::SandPile
            | BuildingType::BonePile
            | BuildingType::WheelbarrowParking
            | BuildingType::RestArea => BuildingCategory::Temporary,
        }
    }

    pub fn required_materials(&self) -> HashMap<ResourceType, u32> {
        let mut materials = HashMap::new();
        match self {
            BuildingType::Wall => {
                materials.insert(ResourceType::Wood, 1);
                materials.insert(ResourceType::StasisMud, 1);
            }
            BuildingType::Door => {
                materials.insert(ResourceType::Wood, 1);
                materials.insert(ResourceType::Bone, 1);
            }
            BuildingType::Floor => {}
            BuildingType::Tank => {
                materials.insert(ResourceType::Wood, 2);
            }
            BuildingType::MudMixer => {
                materials.insert(ResourceType::Wood, 4);
            }
            BuildingType::RestArea => {
                materials.insert(ResourceType::Wood, 5);
            }
            BuildingType::Bridge => {}
            BuildingType::SandPile => {
                materials.insert(ResourceType::Sand, 10);
            }
            BuildingType::BonePile => {
                materials.insert(ResourceType::Bone, 10);
            }
            BuildingType::WheelbarrowParking => {
                materials.insert(ResourceType::Wood, 2);
            }
        }
        materials
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Building {
    pub kind: BuildingType,
    pub is_provisional: bool,
}

#[derive(Component)]
pub struct BridgeMarker;

#[derive(Debug, Clone)]
pub struct FlexibleMaterialRequirement {
    pub accepted_types: Vec<ResourceType>,
    pub required_total: u32,
    pub delivered_total: u32,
}

impl FlexibleMaterialRequirement {
    pub fn is_complete(&self) -> bool {
        self.delivered_total >= self.required_total
    }

    pub fn remaining(&self) -> u32 {
        self.required_total.saturating_sub(self.delivered_total)
    }

    pub fn accepts(&self, resource_type: ResourceType) -> bool {
        self.accepted_types.contains(&resource_type)
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ProvisionalWall {
    pub mud_delivered: bool,
}

#[derive(Component)]
pub struct SandPile;

#[derive(Component)]
pub struct BonePile;

#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct RestArea {
    pub capacity: usize,
}

#[derive(Component)]
pub struct TargetBlueprint(pub Entity);

#[derive(Component)]
pub struct Tree;

#[derive(Component, Clone, Copy, Debug)]
pub struct TreeVariant(pub usize);

#[derive(Component)]
pub struct Rock;

#[derive(Component)]
pub struct ObstaclePosition(pub i32, pub i32);

#[derive(Component)]
pub struct Blueprint {
    pub kind: BuildingType,
    pub progress: f32,
    pub required_materials: HashMap<ResourceType, u32>,
    pub delivered_materials: HashMap<ResourceType, u32>,
    pub flexible_material_requirement: Option<FlexibleMaterialRequirement>,
    pub occupied_grids: Vec<(i32, i32)>,
}

impl Blueprint {
    pub fn new(kind: BuildingType, occupied_grids: Vec<(i32, i32)>) -> Self {
        Self {
            kind,
            progress: 0.0,
            required_materials: kind.required_materials(),
            delivered_materials: HashMap::new(),
            flexible_material_requirement: (kind == BuildingType::Bridge).then_some(
                FlexibleMaterialRequirement {
                    accepted_types: vec![ResourceType::Wood, ResourceType::Rock],
                    required_total: 6,
                    delivered_total: 0,
                },
            ),
            occupied_grids,
        }
    }

    pub fn materials_complete(&self) -> bool {
        if self.kind == BuildingType::Wall {
            let wood_delivered = self
                .delivered_materials
                .get(&ResourceType::Wood)
                .unwrap_or(&0);
            let wood_required = self
                .required_materials
                .get(&ResourceType::Wood)
                .unwrap_or(&1);
            return wood_delivered >= wood_required;
        }

        if let Some(flexible) = &self.flexible_material_requirement {
            return flexible.is_complete();
        }

        for (resource_type, required) in &self.required_materials {
            let delivered = self.delivered_materials.get(resource_type).unwrap_or(&0);
            if delivered < required {
                return false;
            }
        }
        true
    }

    pub fn is_fully_complete(&self) -> bool {
        if let Some(flexible) = &self.flexible_material_requirement {
            return flexible.is_complete();
        }

        for (resource_type, required) in &self.required_materials {
            let delivered = self.delivered_materials.get(resource_type).unwrap_or(&0);
            if delivered < required {
                return false;
            }
        }
        true
    }

    pub fn remaining_material_amount(&self, resource_type: ResourceType) -> u32 {
        if let Some(flexible) = &self.flexible_material_requirement
            && flexible.accepts(resource_type)
        {
            return flexible.remaining();
        }

        let required = *self.required_materials.get(&resource_type).unwrap_or(&0);
        let delivered = *self.delivered_materials.get(&resource_type).unwrap_or(&0);
        required.saturating_sub(delivered)
    }

    pub fn deliver_material(&mut self, resource_type: ResourceType, amount: u32) {
        let mut flexible_delivered = None;
        if let Some(flexible) = self.flexible_material_requirement.as_mut()
            && flexible.accepts(resource_type)
        {
            let delivered_now = amount.min(flexible.remaining());
            flexible.delivered_total += delivered_now;
            flexible_delivered = Some(delivered_now);
        }
        if let Some(delivered_now) = flexible_delivered {
            if delivered_now > 0 {
                *self.delivered_materials.entry(resource_type).or_insert(0) += delivered_now;
            }
            return;
        }

        *self.delivered_materials.entry(resource_type).or_insert(0) += amount;
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct MovePlanned {
    pub task_entity: Entity,
}

#[derive(Component)]
pub struct Designation {
    pub work_type: WorkType,
}

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Priority(pub u32);

#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component, Default)]
pub struct TaskSlots {
    pub max: u32,
}

impl TaskSlots {
    pub fn new(max: u32) -> Self {
        Self { max }
    }
}

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Door {
    pub state: DoorState,
}

impl Default for Door {
    fn default() -> Self {
        Self {
            state: DoorState::Closed,
        }
    }
}

impl Door {
    pub fn is_passable(&self) -> bool {
        self.state != DoorState::Locked
    }

    pub fn is_open(&self) -> bool {
        self.state == DoorState::Open
    }
}

#[derive(Component)]
pub struct DoorCloseTimer {
    pub timer: Timer,
}

impl DoorCloseTimer {
    pub fn new() -> Self {
        Self {
            timer: Timer::from_seconds(DOOR_CLOSE_DELAY_SECS, TimerMode::Once),
        }
    }
}

/// [`Designation`]・[`TaskSlots`]・[`Priority`] をタイルエンティティから一括削除する。
///
/// フェーズ遷移の終端で designation システムが再付与する前にコンポーネントをクリアする。
pub fn remove_tile_task_components(commands: &mut Commands, tile_entities: &[Entity]) {
    for &entity in tile_entities {
        commands
            .entity(entity)
            .remove::<(Designation, TaskSlots, Priority)>();
    }
}
