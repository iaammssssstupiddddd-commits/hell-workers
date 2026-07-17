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

/// Validates resources consumed by rehydration before the live persisted world is despawned.
pub(crate) fn validate_rehydrate_prerequisites(
    world: &World,
) -> Result<(), RehydratePrerequisiteError> {
    let mut missing_resources = Vec::new();

    macro_rules! require_resource {
        ($type:ty) => {
            if !world.contains_resource::<$type>() {
                missing_resources.push(std::any::type_name::<$type>());
            }
        };
    }

    require_resource!(GameAssets);
    require_resource!(Building3dHandles);
    require_resource!(SoulTaskHandles);
    require_resource!(WorldMap);

    let mut invalid_conditions = Vec::new();
    if let Some(game_assets) = world.get_resource::<GameAssets>()
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
