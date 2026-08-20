//! Epoch-aware CPU Light Field to GPU image bridge.

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use hw_core::WorldEpoch;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_infra::lighting::{GridDimensions, digest_hex, pack_rgba8_linear};

use crate::systems::lighting::{
    IndoorLightRuntime, IndoorLightingLifecycleProbe, read_indoor_light_snapshot,
};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndoorLightUploadSet;

pub(crate) const STEADY_UPDATE_CALLS: u64 = 600;
pub(crate) const LIGHT_FIELD_TEXTURE_LABEL: &str = "hell-workers-indoor-light-field";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndoorLightTextureMetrics {
    pub update_calls: u64,
    pub upload_count: u64,
    pub changed_revision_samples: u64,
    pub black_clear_count: u64,
    pub reset_count: u64,
    pub logical_payload_bytes: u64,
    pub staging_bytes: u64,
    pub upload_allocation_events: u64,
    pub upload_allocation_bytes: u64,
    pub old_epoch_uploads: u64,
    pub steady_updates: u64,
    pub steady_uploads: u64,
}

#[derive(Resource, Debug, Clone)]
pub struct IndoorLightTexture {
    handle: Handle<Image>,
    dimensions: GridDimensions,
    uploaded_revision: Option<u64>,
    uploaded_epoch: Option<u64>,
    uploaded_checksum: Option<String>,
    metrics: IndoorLightTextureMetrics,
}

impl IndoorLightTexture {
    pub fn handle(&self) -> &Handle<Image> {
        &self.handle
    }

    pub const fn dimensions(&self) -> GridDimensions {
        self.dimensions
    }

    pub const fn uploaded_revision(&self) -> Option<u64> {
        self.uploaded_revision
    }

    pub const fn uploaded_epoch(&self) -> Option<u64> {
        self.uploaded_epoch
    }

    pub fn uploaded_checksum(&self) -> Option<&str> {
        self.uploaded_checksum.as_deref()
    }

    pub const fn metrics(&self) -> &IndoorLightTextureMetrics {
        &self.metrics
    }

    #[cfg(feature = "profiling-renderdoc")]
    pub(crate) fn begin_renderdoc_measurement(&mut self) {
        let logical_payload_bytes = self.metrics.logical_payload_bytes;
        let staging_bytes = self.metrics.staging_bytes;
        self.metrics = IndoorLightTextureMetrics {
            logical_payload_bytes,
            staging_bytes,
            ..default()
        };
        self.uploaded_revision = None;
        self.uploaded_epoch = None;
        self.uploaded_checksum = None;
    }
}

fn staging_bytes(dimensions: GridDimensions) -> u64 {
    let bytes_per_row = u64::from(dimensions.width()) * 4;
    let padded_row = bytes_per_row.div_ceil(256) * 256;
    padded_row * u64::from(dimensions.height())
}

fn make_light_field_image(dimensions: GridDimensions, data: Vec<u8>) -> Image {
    let expected_len = dimensions.area() * 4;
    assert_eq!(data.len(), expected_len, "Light Field RGBA payload size");
    let mut image = Image::new(
        Extent3d {
            width: u32::from(dimensions.width()),
            height: u32::from(dimensions.height()),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        default(),
    );
    image.texture_descriptor.label = Some(LIGHT_FIELD_TEXTURE_LABEL);
    #[cfg(feature = "profiling-renderdoc")]
    {
        image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
    }
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        mipmap_filter: ImageFilterMode::Nearest,
        ..default()
    });
    image
}

pub fn init_indoor_light_texture_system(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let dimensions = GridDimensions::new(
        u16::try_from(MAP_WIDTH).expect("MAP_WIDTH fits Light Field dimensions"),
        u16::try_from(MAP_HEIGHT).expect("MAP_HEIGHT fits Light Field dimensions"),
    )
    .expect("production map dimensions satisfy the Light Field contract");
    let handle = images.add(make_light_field_image(
        dimensions,
        vec![0; dimensions.area() * 4],
    ));
    commands.insert_resource(IndoorLightTexture {
        handle,
        dimensions,
        uploaded_revision: None,
        uploaded_epoch: None,
        uploaded_checksum: None,
        metrics: IndoorLightTextureMetrics {
            logical_payload_bytes: u64::try_from(dimensions.area() * 4)
                .expect("Light Field payload fits u64"),
            staging_bytes: staging_bytes(dimensions),
            ..default()
        },
    });
}

pub fn upload_indoor_light_texture_system(
    runtime: Res<IndoorLightRuntime>,
    world_epoch: Res<WorldEpoch>,
    mut lifecycle_probe: ResMut<IndoorLightingLifecycleProbe>,
    mut texture: ResMut<IndoorLightTexture>,
    mut images: ResMut<Assets<Image>>,
) {
    upload_indoor_light_texture_once(
        &runtime,
        *world_epoch,
        &mut lifecycle_probe,
        &mut texture,
        &mut images,
    );
}

fn upload_indoor_light_texture_once(
    runtime: &IndoorLightRuntime,
    current_epoch: WorldEpoch,
    lifecycle_probe: &mut IndoorLightingLifecycleProbe,
    texture: &mut IndoorLightTexture,
    images: &mut Assets<Image>,
) {
    texture.metrics.update_calls = texture.metrics.update_calls.saturating_add(1);
    let Some(snapshot) =
        read_indoor_light_snapshot(runtime, current_epoch, current_epoch, lifecycle_probe)
    else {
        return;
    };
    if texture.uploaded_revision == Some(snapshot.field_revision())
        && texture.uploaded_epoch == Some(current_epoch.get())
    {
        texture.metrics.steady_updates = texture
            .metrics
            .steady_updates
            .saturating_add(1)
            .min(STEADY_UPDATE_CALLS);
        return;
    }

    let dimensions = snapshot.dimensions();
    let revision = snapshot.field_revision();
    let packed = pack_rgba8_linear(snapshot);
    texture.metrics.upload_allocation_events =
        texture.metrics.upload_allocation_events.saturating_add(1);
    texture.metrics.upload_allocation_bytes = texture
        .metrics
        .upload_allocation_bytes
        .saturating_add(u64::try_from(packed.len()).expect("Light Field payload fits u64"));
    let Some(mut image) = images.get_mut(&texture.handle) else {
        return;
    };
    if texture.dimensions == dimensions {
        image.data = Some(packed);
    } else {
        *image = make_light_field_image(dimensions, packed);
        texture.dimensions = dimensions;
    }
    texture.uploaded_revision = Some(revision);
    texture.uploaded_epoch = Some(current_epoch.get());
    texture.uploaded_checksum = Some(digest_hex(snapshot.field_checksum()));
    if texture.metrics.steady_updates > 0 {
        texture.metrics.steady_uploads = texture.metrics.steady_uploads.saturating_add(1);
    }
    texture.metrics.steady_updates = 0;
    texture.metrics.upload_count = texture.metrics.upload_count.saturating_add(1);
    texture.metrics.changed_revision_samples =
        texture.metrics.changed_revision_samples.saturating_add(1);
    texture.metrics.logical_payload_bytes =
        u64::try_from(dimensions.area() * 4).expect("Light Field payload fits u64");
    texture.metrics.staging_bytes = staging_bytes(dimensions);
}

#[cfg(feature = "profiling-renderdoc")]
pub(crate) fn collect_renderdoc_steady_window(
    runtime: &IndoorLightRuntime,
    current_epoch: WorldEpoch,
    lifecycle_probe: &mut IndoorLightingLifecycleProbe,
    texture: &mut IndoorLightTexture,
    images: &mut Assets<Image>,
) {
    texture.begin_renderdoc_measurement();
    for _ in 0..=STEADY_UPDATE_CALLS {
        upload_indoor_light_texture_once(runtime, current_epoch, lifecycle_probe, texture, images);
    }
}

pub fn reset_indoor_light_texture_for_world_replace(world: &mut World) {
    let Some(texture) = world.get_resource::<IndoorLightTexture>() else {
        return;
    };
    let handle = texture.handle.clone();
    let dimensions = texture.dimensions;

    if let Some(mut images) = world.get_resource_mut::<Assets<Image>>()
        && let Some(mut image) = images.get_mut(&handle)
    {
        let expected_len = dimensions.area() * 4;
        if let Some(data) = image.data.as_mut()
            && data.len() == expected_len
        {
            data.fill(0);
        } else {
            *image = make_light_field_image(dimensions, vec![0; expected_len]);
        }
    }

    if let Some(mut texture) = world.get_resource_mut::<IndoorLightTexture>() {
        texture.uploaded_revision = None;
        texture.uploaded_epoch = None;
        texture.uploaded_checksum = None;
        texture.metrics.steady_updates = 0;
        texture.metrics.black_clear_count = texture.metrics.black_clear_count.saturating_add(1);
        texture.metrics.reset_count = texture.metrics.reset_count.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "profiling-renderdoc")]
    fn collect_renderdoc_steady_window_system(
        runtime: Res<IndoorLightRuntime>,
        world_epoch: Res<WorldEpoch>,
        mut lifecycle_probe: ResMut<IndoorLightingLifecycleProbe>,
        mut texture: ResMut<IndoorLightTexture>,
        mut images: ResMut<Assets<Image>>,
    ) {
        collect_renderdoc_steady_window(
            &runtime,
            *world_epoch,
            &mut lifecycle_probe,
            &mut texture,
            &mut images,
        );
    }

    #[test]
    fn canonical_image_is_linear_nearest_and_has_padded_staging_size() {
        let dimensions = GridDimensions::new(100, 100).unwrap();
        let image = make_light_field_image(dimensions, vec![0; dimensions.area() * 4]);

        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(image.texture_descriptor.size.width, 100);
        assert_eq!(image.texture_descriptor.size.height, 100);
        assert_eq!(
            image.texture_descriptor.label,
            Some(LIGHT_FIELD_TEXTURE_LABEL)
        );
        assert_eq!(image.data.as_deref().map(<[u8]>::len), Some(40_000));
        assert_eq!(staging_bytes(dimensions), 51_200);
        #[cfg(feature = "profiling-renderdoc")]
        assert!(
            image
                .texture_descriptor
                .usage
                .contains(bevy::render::render_resource::TextureUsages::COPY_SRC)
        );
    }

    #[test]
    fn reset_blackens_the_existing_handle_and_invalidates_epoch() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.add_systems(Startup, init_indoor_light_texture_system);
        app.update();

        let handle = app
            .world()
            .resource::<IndoorLightTexture>()
            .handle()
            .clone();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&handle)
            .unwrap()
            .data
            .as_mut()
            .unwrap()
            .fill(255);
        reset_indoor_light_texture_for_world_replace(app.world_mut());

        let texture = app.world().resource::<IndoorLightTexture>();
        assert_eq!(texture.uploaded_epoch(), None);
        assert_eq!(texture.uploaded_revision(), None);
        assert_eq!(texture.metrics().black_clear_count, 1);
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&handle)
                .unwrap()
                .data
                .as_ref()
                .unwrap()
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[cfg(feature = "profiling-renderdoc")]
    #[test]
    fn renderdoc_window_runs_the_production_upload_path_without_render_frames() {
        use std::collections::HashMap;

        use hw_energy::PowerSupplyState;
        use hw_jobs::{Building, BuildingType};
        use hw_world::{DoorLockToggleRequest, RoomTileLookup, WorldMap};

        use crate::plugins::lighting::IndoorLightingPlugin;
        use crate::systems::GameSystemSet;
        use crate::systems::lighting::{IndoorLightingRebuildSet, RadialLightEmitter};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DoorLockToggleRequest>()
            .init_resource::<WorldMap>()
            .init_resource::<RoomTileLookup>()
            .init_resource::<Assets<Image>>()
            .configure_sets(
                Update,
                (
                    GameSystemSet::Input,
                    GameSystemSet::Spatial,
                    GameSystemSet::Logic,
                    GameSystemSet::PreActor,
                    GameSystemSet::Actor,
                    GameSystemSet::PostActor,
                    GameSystemSet::Visual,
                    GameSystemSet::Interface,
                )
                    .chain(),
            )
            .add_plugins(IndoorLightingPlugin)
            .add_systems(Startup, init_indoor_light_texture_system)
            .add_systems(
                Update,
                collect_renderdoc_steady_window_system.after(IndoorLightingRebuildSet),
            );

        let grid = (10, 10);
        app.world_mut()
            .resource_mut::<RoomTileLookup>()
            .replace(HashMap::from([(grid, Entity::PLACEHOLDER)]));
        let world = WorldMap::grid_to_world(grid.0, grid.1);
        app.world_mut().spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(world.extend(0.0)),
            RadialLightEmitter::outdoor_lamp(grid),
            PowerSupplyState::Supplied,
        ));

        app.update();

        let metrics = app.world().resource::<IndoorLightTexture>().metrics();
        assert_eq!(metrics.update_calls, STEADY_UPDATE_CALLS + 1);
        assert_eq!(metrics.upload_count, 1);
        assert_eq!(metrics.changed_revision_samples, 1);
        assert_eq!(metrics.steady_updates, STEADY_UPDATE_CALLS);
        assert_eq!(metrics.steady_uploads, 0);
    }
}
