//! 仲裁処理の型定義

use std::cmp::Ordering;

use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use bevy::prelude::*;

#[derive(Clone)]
pub struct BatchCandidate {
    pub request_entity: Entity,
    pub items: Vec<Entity>,
    pub source_pos: Vec2,
    pub destination: WheelbarrowDestination,
    pub group_cells: Vec<Entity>,
    pub hard_min: usize,
    pub pending_for: f64,
    pub is_small_batch: bool,
}

#[derive(Clone, Copy)]
pub struct FreeItemSnapshot {
    pub entity: Entity,
    pub pos: Vec2,
    pub resource_type: ResourceType,
    pub owner: Option<Entity>,
    pub is_ground: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemBucketKey {
    Resource(ResourceType),
    ResourceOwnerGround {
        resource_type: ResourceType,
        owner: Option<Entity>,
    },
}

pub struct RequestEvalContext {
    pub request_entity: Entity,
    pub request_pos: Vec2,
    pub resource_type: ResourceType,
    pub destination: WheelbarrowDestination,
    pub max_items: usize,
    pub hard_min: usize,
    pub pending_for: f64,
    pub priority: u32,
    pub bucket_key: ItemBucketKey,
}

#[derive(Clone, Copy)]
pub struct NearbyItem {
    pub entity: Entity,
    pub pos: Vec2,
    pub dist_sq: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct HeapEntry {
    pub snapshot_idx: usize,
    pub dist_sq: f32,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot_idx == other.snapshot_idx
            && self.dist_sq.total_cmp(&other.dist_sq) == Ordering::Equal
    }
}

impl Eq for HeapEntry {}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.dist_sq
            .total_cmp(&other.dist_sq)
            .then_with(|| self.snapshot_idx.cmp(&other.snapshot_idx))
    }
}
