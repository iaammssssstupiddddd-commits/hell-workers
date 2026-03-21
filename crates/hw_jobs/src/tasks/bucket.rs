use bevy::prelude::*;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct BucketTransportData {
    pub bucket: Entity,
    pub source: BucketTransportSource,
    pub destination: BucketTransportDestination,
    pub amount: u32,
    pub phase: BucketTransportPhase,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub enum BucketTransportSource {
    River,
    Tank { tank: Entity, needs_fill: bool },
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub enum BucketTransportDestination {
    Tank(Entity),
    Mixer(Entity),
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum BucketTransportPhase {
    #[default]
    GoingToBucket,
    GoingToSource,
    Filling {
        progress: f32,
    },
    GoingToDestination,
    Pouring {
        progress: f32,
    },
    ReturningBucket,
}

impl BucketTransportData {
    pub fn source_entity(&self) -> Entity {
        match self.source {
            BucketTransportSource::River => self.bucket,
            BucketTransportSource::Tank { tank, .. } => tank,
        }
    }

    pub fn destination_entity(&self) -> Entity {
        match self.destination {
            BucketTransportDestination::Tank(entity) => entity,
            BucketTransportDestination::Mixer(entity) => entity,
        }
    }

    pub fn should_reserve_bucket_source(&self) -> bool {
        matches!(self.phase, BucketTransportPhase::GoingToBucket)
    }

    pub fn should_reserve_tank_source(&self) -> bool {
        match self.source {
            BucketTransportSource::Tank {
                needs_fill: true, ..
            } => matches!(
                self.phase,
                BucketTransportPhase::GoingToBucket
                    | BucketTransportPhase::GoingToSource
                    | BucketTransportPhase::Filling { .. }
            ),
            BucketTransportSource::Tank { .. } | BucketTransportSource::River => false,
        }
    }

    pub fn should_reserve_mixer_destination(&self) -> bool {
        matches!(self.destination, BucketTransportDestination::Mixer(_))
            && !matches!(self.phase, BucketTransportPhase::ReturningBucket)
    }

    pub fn tank_source_entity(&self) -> Option<Entity> {
        match self.source {
            BucketTransportSource::Tank { tank, .. } => Some(tank),
            BucketTransportSource::River => None,
        }
    }
}
