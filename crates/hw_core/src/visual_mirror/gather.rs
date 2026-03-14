use bevy::prelude::*;

/// Marker attached to entities that have both a `Designation` and are a `Tree` or `Rock`.
/// `hw_jobs` Observers sync this; `hw_visual` reads it for gather highlight rendering.
#[derive(Component)]
pub struct GatherHighlightMarker;

/// Mirror of `hw_jobs::RestArea` carrying only the data `hw_visual` needs.
/// Attached by an `OnAdd<RestArea>` Observer in `hw_jobs`.
#[derive(Component)]
pub struct RestAreaVisual {
    pub capacity: usize,
}
