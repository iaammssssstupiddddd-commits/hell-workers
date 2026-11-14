//! Shared capture metadata for two closed, independently admitted art sessions.
//! This resource grants no candidate, production, or performance permission.
use std::path::Path;

use bevy::prelude::*;
use serde::{Serialize, Serializer};

use super::{
    BuildingArtGroup, BuildingAssetKind, acceptance::BuildingArtView, group_art::GroupArtSession,
    release_gallery::ReleaseArtSession,
};

#[derive(Clone, Debug, Resource)]
pub(crate) enum GalleryCaptureSession {
    Group(GroupArtSession),
    Released(ReleaseArtSession),
    TankPilot(super::tank_illustration_pilot::TankIllustrationPilot),
}

impl Serialize for GalleryCaptureSession {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Group(value) => value.serialize(serializer),
            Self::Released(value) => value.serialize(serializer),
            Self::TankPilot(value) => value.serialize(serializer),
        }
    }
}

impl GalleryCaptureSession {
    pub(crate) fn contract(&self) -> &str {
        match self {
            Self::Group(value) => &value.contract_id,
            Self::Released(value) => &value.contract_id,
            Self::TankPilot(value) => &value.contract_id,
        }
    }

    pub(crate) fn nonce(&self) -> &str {
        match self {
            Self::Group(value) => &value.nonce,
            Self::Released(value) => &value.nonce,
            Self::TankPilot(value) => &value.gallery.nonce,
        }
    }

    pub(crate) fn status_path(&self) -> &Path {
        match self {
            Self::Group(value) => &value.status_path,
            Self::Released(value) => &value.status_path,
            Self::TankPilot(value) => &value.gallery.status_path,
        }
    }

    pub(crate) fn view(&self) -> BuildingArtView {
        match self {
            Self::Group(value) => value.view,
            Self::Released(value) => value.view,
            Self::TankPilot(value) => value.gallery.view,
        }
    }

    pub(crate) fn quality(&self) -> &str {
        match self {
            Self::Group(value) => &value.quality,
            Self::Released(value) => &value.quality,
            Self::TankPilot(value) => &value.gallery.quality,
        }
    }

    pub(crate) fn scale_factor(&self) -> f32 {
        match self {
            Self::Group(value) => value.scale_factor,
            Self::Released(value) => value.scale_factor,
            Self::TankPilot(value) => value.gallery.scale_factor,
        }
    }

    pub(crate) fn group(&self) -> Option<BuildingArtGroup> {
        match self {
            Self::Group(value) => Some(value.group()),
            Self::Released(_) | Self::TankPilot(_) => None,
        }
    }

    pub(crate) fn check_door(
        &self,
        identity: &crate::assets::door_asset_set::DoorAssetSetIdentity,
    ) -> Result<(), String> {
        match self {
            Self::Group(_) => Ok(()),
            Self::Released(value) => value.check_door(identity),
            Self::TankPilot(value) => value.gallery.check_door(identity),
        }
    }

    pub(crate) fn target(&self, kind: BuildingAssetKind) -> bool {
        match self {
            Self::Group(value) => value.target(kind),
            Self::Released(value) => value.target(kind),
            Self::TankPilot(value) => value.gallery.target(kind),
        }
    }
}
