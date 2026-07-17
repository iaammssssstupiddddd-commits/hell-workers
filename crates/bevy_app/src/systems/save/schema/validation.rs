use std::fmt;

use super::*;

/// Error returned when a deserialized DynamicWorld cannot satisfy the durable
/// resource contract required by the live simulation and rehydrate steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicWorldSchemaError {
    pub(super) missing_resources: Vec<&'static str>,
    pub(super) unsupported_resources: Vec<String>,
    pub(super) unsupported_components: Vec<String>,
    pub(super) rootless_entities: Vec<Entity>,
}

impl fmt::Display for DynamicWorldSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut reasons = Vec::new();
        if !self.missing_resources.is_empty() {
            reasons.push(format!(
                "missing required persisted resource(s): {}",
                self.missing_resources.join(", ")
            ));
        }
        if !self.unsupported_resources.is_empty() {
            reasons.push(format!(
                "unsupported resource(s): {}",
                self.unsupported_resources.join(", ")
            ));
        }
        if !self.unsupported_components.is_empty() {
            reasons.push(format!(
                "unsupported component(s): {}",
                self.unsupported_components.join(", ")
            ));
        }
        if !self.rootless_entities.is_empty() {
            reasons.push(format!(
                "entity or entities without a persisted root marker: {}",
                self.rootless_entities.len()
            ));
        }
        write!(
            formatter,
            "save body violates the persisted schema: {}",
            reasons.join("; ")
        )
    }
}

/// Ensures a deserialized save carries every durable resource required to
/// replace the current simulation world and does not contain allow-list-external
/// types. This runs before any live despawn.
pub(crate) fn validate_persisted_world(
    dynamic_world: &DynamicWorld,
) -> Result<(), DynamicWorldSchemaError> {
    let mut missing_resources = Vec::new();
    let mut unsupported_resources = Vec::new();
    let mut unsupported_components = Vec::new();
    let mut rootless_entities = Vec::new();

    macro_rules! require_resource {
        ($type:ty) => {
            if !dynamic_world.resources.iter().any(|resource| {
                resource
                    .get_represented_type_info()
                    .is_some_and(|info| info.type_id() == std::any::TypeId::of::<$type>())
            }) {
                missing_resources.push(std::any::type_name::<$type>());
            }
        };
    }

    for_each_persisted_resource!(require_resource);

    macro_rules! is_persisted_resource {
        ($type_id:expr) => {{
            let mut allowed = false;
            macro_rules! matches_type {
                ($type:ty) => {
                    allowed |= $type_id == std::any::TypeId::of::<$type>();
                };
            }
            for_each_persisted_resource!(matches_type);
            allowed
        }};
    }
    macro_rules! is_persisted_component {
        ($type_id:expr) => {{
            let mut allowed = false;
            macro_rules! matches_type {
                ($type:ty) => {
                    allowed |= $type_id == std::any::TypeId::of::<$type>();
                };
            }
            for_each_persisted_component!(matches_type);
            for_each_external_registered_component!(matches_type);
            allowed
        }};
    }
    macro_rules! is_root_marker {
        ($type_id:expr) => {{
            let mut root_marker = false;
            macro_rules! matches_type {
                ($type:ty) => {
                    root_marker |= $type_id == std::any::TypeId::of::<$type>();
                };
            }
            for_each_root_marker!(matches_type);
            root_marker
        }};
    }

    for resource in &dynamic_world.resources {
        let allowed = resource
            .get_represented_type_info()
            .is_some_and(|info| is_persisted_resource!(info.type_id()));
        if !allowed {
            unsupported_resources.push(resource.reflect_type_path().to_string());
        }
    }
    for entity in &dynamic_world.entities {
        let has_root_marker = entity.components.iter().any(|component| {
            component
                .get_represented_type_info()
                .is_some_and(|info| is_root_marker!(info.type_id()))
        });
        if !has_root_marker {
            rootless_entities.push(entity.entity);
        }
        for component in &entity.components {
            let allowed = component
                .get_represented_type_info()
                .is_some_and(|info| is_persisted_component!(info.type_id()));
            if !allowed {
                unsupported_components.push(component.reflect_type_path().to_string());
            }
        }
    }

    unsupported_resources.sort();
    unsupported_resources.dedup();
    unsupported_components.sort();
    unsupported_components.dedup();

    if missing_resources.is_empty()
        && unsupported_resources.is_empty()
        && unsupported_components.is_empty()
        && rootless_entities.is_empty()
    {
        Ok(())
    } else {
        Err(DynamicWorldSchemaError {
            missing_resources,
            unsupported_resources,
            unsupported_components,
            rootless_entities,
        })
    }
}
