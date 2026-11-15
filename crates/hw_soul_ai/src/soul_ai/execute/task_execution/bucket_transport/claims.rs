//! Deferred entity commands are invisible to later actors in this execution pass.
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct WaterTransferClaims {
    pub buckets: HashSet<Entity>,
    pub taken: HashSet<Entity>,
    pub deposited: HashMap<Entity, usize>,
}
