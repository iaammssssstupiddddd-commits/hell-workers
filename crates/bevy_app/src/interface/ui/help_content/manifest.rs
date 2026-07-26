use super::providers;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum HelpOwnerId {
    RootOnboarding,
    InputAndCamera,
    FamiliarManagement,
    OrdersAndBuilding,
    SoulEnergy,
    PersistenceAndSettings,
}

impl HelpOwnerId {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RootOnboarding => "root-onboarding",
            Self::InputAndCamera => "input-camera",
            Self::FamiliarManagement => "familiar-management",
            Self::OrdersAndBuilding => "orders-building",
            Self::SoulEnergy => "soul-energy",
            Self::PersistenceAndSettings => "persistence-settings",
        }
    }
}

macro_rules! player_help_features {
    (
        $(
            $variant:ident => {
                id: $id:literal,
                owner: $owner:ident,
                section_order: $section_order:literal,
                topic_order: $topic_order:literal,
                provider: $provider:path
            }
        ),+ $(,)?
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) enum PlayerFeatureId {
            $($variant),+
        }

        impl PlayerFeatureId {
            pub(crate) const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub(crate) const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $id),+
                }
            }
        }

        pub(crate) fn feature_specs() -> Vec<FeatureSpec> {
            vec![
                $(
                    FeatureSpec {
                        feature: PlayerFeatureId::$variant,
                        owner: HelpOwnerId::$owner,
                        section_order: $section_order,
                        topic_order: $topic_order,
                        provider: $provider,
                    }
                ),+
            ]
        }
    };
}

pub(crate) struct FeatureSpec {
    pub(crate) feature: PlayerFeatureId,
    pub(crate) owner: HelpOwnerId,
    pub(crate) section_order: u16,
    pub(crate) topic_order: u16,
    pub(crate) provider: super::ProviderFn,
}

player_help_features! {
    GettingStarted => {
        id: "getting-started",
        owner: RootOnboarding,
        section_order: 10,
        topic_order: 10,
        provider: providers::getting_started::getting_started
    },
    CameraAndSelection => {
        id: "camera-selection",
        owner: InputAndCamera,
        section_order: 20,
        topic_order: 10,
        provider: providers::camera_selection::camera_and_selection
    },
    TimeAndHelp => {
        id: "time-help",
        owner: InputAndCamera,
        section_order: 20,
        topic_order: 20,
        provider: providers::camera_selection::time_and_help
    },
    EntityListAndSquads => {
        id: "entity-list-squads",
        owner: FamiliarManagement,
        section_order: 30,
        topic_order: 10,
        provider: providers::familiars::entity_list_and_squads
    },
    FamiliarCommands => {
        id: "familiar-commands",
        owner: FamiliarManagement,
        section_order: 30,
        topic_order: 20,
        provider: providers::familiars::familiar_commands
    },
    SoulEnergy => {
        id: "soul-energy",
        owner: SoulEnergy,
        section_order: 30,
        topic_order: 30,
        provider: providers::soul_energy::soul_energy
    },
    InfoPanel => {
        id: "info-panel",
        owner: FamiliarManagement,
        section_order: 30,
        topic_order: 40,
        provider: providers::familiars::info_panel
    },
    OrdersAndAreas => {
        id: "orders-areas",
        owner: OrdersAndBuilding,
        section_order: 40,
        topic_order: 10,
        provider: providers::orders_building_zones::orders_and_areas
    },
    BuildingZonesDream => {
        id: "building-zones-dream",
        owner: OrdersAndBuilding,
        section_order: 40,
        topic_order: 20,
        provider: providers::orders_building_zones::building_zones_dream
    },
    TaskDashboard => {
        id: "task-dashboard",
        owner: OrdersAndBuilding,
        section_order: 40,
        topic_order: 30,
        provider: providers::orders_building_zones::task_dashboard
    },
    SaveSettingsNotifications => {
        id: "save-settings-notifications",
        owner: PersistenceAndSettings,
        section_order: 50,
        topic_order: 10,
        provider: providers::save_settings_notifications::save_settings_notifications
    }
}
