use std::fmt;

use super::*;

/// Resources required to rebuild runtime shells after a persistent world replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RehydratePrerequisiteError {
    pub(super) missing_resources: Vec<&'static str>,
    pub(super) invalid_conditions: Vec<&'static str>,
}

impl fmt::Display for RehydratePrerequisiteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot rehydrate: missing resource(s): {}; invalid condition(s): {}",
            self.missing_resources.join(", "),
            self.invalid_conditions.join(", ")
        )
    }
}

fn validate_resources(
    world: &World,
    include_simulation: bool,
    include_presentation: bool,
) -> Result<(), RehydratePrerequisiteError> {
    let mut missing_resources = Vec::new();

    macro_rules! require_resource {
        ($type:ty) => {
            if !world.contains_resource::<$type>() {
                missing_resources.push(std::any::type_name::<$type>());
            }
        };
    }

    if include_presentation {
        require_resource!(GameAssets);
        require_resource!(Building3dHandles);
        require_resource!(SoulTaskHandles);
        require_resource!(Time<Virtual>);
    }
    if include_simulation {
        require_resource!(WorldMap);
    }

    let mut invalid_conditions = Vec::new();
    if include_presentation
        && let Some(game_assets) = world.get_resource::<GameAssets>()
        && game_assets.trees.is_empty()
    {
        invalid_conditions.push("GameAssets.trees must not be empty");
    }

    if missing_resources.is_empty() && invalid_conditions.is_empty() {
        Ok(())
    } else {
        Err(RehydratePrerequisiteError {
            missing_resources,
            invalid_conditions,
        })
    }
}

pub(super) fn validate_presentation_prerequisites(world: &World) -> Result<(), String> {
    validate_resources(world, false, true).map_err(|error| error.to_string())
}

#[cfg(test)]
pub(in crate::systems::save) fn validate_rehydrate_prerequisites(
    world: &World,
) -> Result<(), RehydratePrerequisiteError> {
    validate_resources(world, true, true)
}
