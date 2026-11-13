//! Released Door preview audit. Supports are explicitly seeded once, while
//! catalog selection, ghost movement and Blueprint creation require OS input.
//! This observer grants no art, performance, construction or release authority.
use super::*;
use bevy::input::{keyboard::KeyboardInput, mouse::MouseButtonInput};
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_world::WorldMap;

mod consumers;
mod input;
mod preparation;

const SCOPE: &str = "ordinary-door-release-planning-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    EastWest,
    NorthSouth,
    Tie,
}

impl Case {
    fn parse(case: &str) -> Option<Self> {
        match case {
            "door-release-east-west" => Some(Self::EastWest),
            "door-release-north-south" => Some(Self::NorthSouth),
            "door-release-tie" => Some(Self::Tie),
            _ => None,
        }
    }

    fn supports(self, (x, y): (i32, i32)) -> Vec<(i32, i32)> {
        match self {
            Self::EastWest => vec![(x - 1, y), (x + 1, y)],
            Self::NorthSouth => vec![(x, y - 1), (x, y + 1)],
            Self::Tie => vec![(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)],
        }
    }

    fn expected_axis(self) -> &'static str {
        match self {
            Self::NorthSouth => "NorthSouth",
            _ => "EastWest",
        }
    }
}

fn current_case() -> Option<Case> {
    std::env::var("HW_NATIVE_UI_CASE")
        .ok()
        .as_deref()
        .and_then(Case::parse)
}

pub(super) fn enabled() -> bool {
    current_case().is_some()
}

pub(super) fn prepare(world: &mut World) {
    if let Err(reason) = preparation::prepare(world) {
        error!("Door release preparation failed: {reason}");
        world.write_message(AppExit::error());
    }
}

pub(super) fn record_frame(world: &mut World, observer: &UiObserver) -> Result<(), String> {
    input::record_frame(world, observer)
}

pub(super) fn snapshot(world: &mut World) -> Value {
    match consumers::snapshot(world) {
        Ok(value) => value,
        Err(reason) => json!({"scope": SCOPE, "failure": reason}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn door_cases_are_closed_and_tie_has_only_four_cardinal_supports() {
        assert_eq!(Case::parse("door-release-east-west"), Some(Case::EastWest));
        assert_eq!(
            Case::parse("door-release-north-south"),
            Some(Case::NorthSouth)
        );
        assert_eq!(Case::parse("door-release-tie"), Some(Case::Tie));
        for case in [
            "Door",
            "bridge-planning",
            "m5-lamp-planning",
            "navigation",
            "",
        ] {
            assert_eq!(Case::parse(case), None);
        }
        assert_eq!(
            Case::Tie.supports((4, 5)),
            vec![(3, 5), (5, 5), (4, 4), (4, 6)]
        );
        assert_eq!(Case::Tie.expected_axis(), "EastWest");
        assert_eq!(Case::NorthSouth.expected_axis(), "NorthSouth");
    }
}
