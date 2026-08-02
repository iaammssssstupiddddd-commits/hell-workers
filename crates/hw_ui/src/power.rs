//! Game-agnostic power inspection and edit values.
//!
//! The root application converts its energy-domain types at the UI boundary.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PowerPriorityValue {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAllocationModeValue {
    LegacyAllOrNone,
    PriorityPrefix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerShedReasonValue {
    InsufficientGeneration,
    RestoreMargin,
    LegacyGlobalDeficit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerSupplyStateValue {
    Supplied,
    Shed { reason: PowerShedReasonValue },
    Disconnected,
    InvalidDemand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerInspectionRole {
    Generator,
    Consumer,
}
