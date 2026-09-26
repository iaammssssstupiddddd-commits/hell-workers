use std::{
    path::Path,
    sync::{Arc, Weak},
    thread,
    time::Duration,
};

use bevy::asset::{
    AssetMetaCheck, LoadState, StrongHandle,
    io::{
        AssetSourceBuilder, AssetSourceId,
        memory::{Dir, MemoryAssetReader},
    },
};
use bevy::gltf::{GltfAssetLabel, GltfPlugin};
use bevy::image::{CompressedImageFormats, ImageLoader};
use bevy::mesh::MeshPlugin;
use bevy::world_serialization::WorldSerializationPlugin;

use super::super::tests::{artifact, fixture, receipt_bytes, seal};
use super::super::validation::canonical;
use super::super::{BuildingAssetAuthority, BuildingAssetSetLoader};
use super::*;

// A complete 1x1 RGBA PNG, including zlib stream and CRCs.
const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 255, 255, 255, 127, 0,
    9, 251, 3, 253, 42, 134, 227, 138, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn glb() -> Vec<u8> {
    // A triangle with an embedded position buffer, no external dependencies.
    let mut json = serde_json::to_vec(&serde_json::json!({
        "asset": {"version": "2.0"},
        "buffers": [{"byteLength": 36}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3,
            "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}]
    }))
    .unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let positions: [f32; 9] = [0., 0., 0., 1., 0., 0., 0., 1., 0.];
    let bin: Vec<_> = positions.into_iter().flat_map(f32::to_le_bytes).collect();
    let mut bytes = b"glTF".to_vec();
    bytes.extend(2_u32.to_le_bytes());
    bytes.extend((28 + json.len() as u32 + bin.len() as u32).to_le_bytes());
    bytes.extend((json.len() as u32).to_le_bytes());
    bytes.extend(b"JSON");
    bytes.extend(json);
    bytes.extend((bin.len() as u32).to_le_bytes());
    bytes.extend(b"BIN\0");
    bytes.extend(bin);
    bytes
}

fn manifest(kind: BuildingAssetKind, generation: u64) -> BuildingAssetSetManifest {
    let mut manifest = fixture(kind, BuildingAssetAuthority::ReleaseApproved);
    let mesh_bytes = glb();
    manifest.identity.generation = generation;
    for preview in [&mut manifest.world_preview, &mut manifest.catalog_preview] {
        preview.canvas_px = [1, 1];
        preview.anchor_px = [0.5, 0.5];
    }
    for record in &mut manifest.artifacts {
        let mesh = record.role.starts_with("mesh:");
        *record = artifact(
            &manifest.identity,
            &record.role,
            if mesh { "glb" } else { "png" },
            if mesh { &mesh_bytes } else { PNG },
        );
    }
    seal(&mut manifest);
    manifest
}

fn provision(
    dir: &Dir,
    manifest: &BuildingAssetSetManifest,
    replacement: Option<(&str, &[u8])>,
) -> String {
    let path = format!(
        "{}-{}.buildingset",
        manifest.identity.kind.slug(),
        manifest.identity.generation
    );
    dir.insert_asset(Path::new(&path), canonical(manifest).unwrap());
    for record in &manifest.artifacts {
        let payload = if let Some((_, bytes)) = replacement.filter(|(role, _)| *role == record.role)
        {
            bytes.to_vec()
        } else if record.role.starts_with("mesh:") {
            glb()
        } else {
            PNG.to_vec()
        };
        dir.insert_asset(Path::new(&record.path), payload);
    }
    dir.insert_asset(
        Path::new(&manifest.receipt.as_ref().unwrap().path),
        receipt_bytes(manifest),
    );
    path
}

fn app(dir: &Dir) -> App {
    let mut app = App::new();
    let root = dir.clone();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: root.clone() })),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            meta_check: AssetMetaCheck::Never,
            ..default()
        },
        WorldSerializationPlugin,
        MeshPlugin,
        GltfPlugin::default(),
    ))
    .init_asset::<Image>()
    .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE))
    .init_asset::<BuildingAssetSetManifest>()
    .init_asset_loader::<BuildingAssetSetLoader>();
    app.finish();
    app.cleanup();
    app
}

fn poll(app: &App, pool: &mut BuildingAssetPool) {
    let world = app.world();
    pool.poll(
        world.resource(),
        world.resource(),
        world.resource(),
        world.resource(),
    );
}

fn until(app: &mut App, mut predicate: impl FnMut(&App) -> bool) {
    for _ in 0..1000 {
        app.update();
        if predicate(app) {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("asset load did not settle");
}

fn settle(app: &mut App, pool: &mut BuildingAssetPool, kind: BuildingAssetKind) {
    until(app, |app| {
        poll(app, pool);
        pool.pending(kind).is_none()
    });
}

fn request(
    app: &App,
    pool: &mut BuildingAssetPool,
    manifest: &BuildingAssetSetManifest,
    path: &str,
) {
    assert_eq!(
        pool.request(manifest.identity.clone(), path, app.world().resource()),
        BuildingAssetRequest::Started
    );
}

fn weak<A: Asset>(handle: &Handle<A>) -> Weak<StrongHandle> {
    let Handle::Strong(handle) = handle else {
        panic!("expected strong handle")
    };
    Arc::downgrade(handle)
}

fn references(
    app: &App,
    pool: &BuildingAssetPool,
    manifest: &BuildingAssetSetManifest,
) -> Vec<Weak<StrongHandle>> {
    let pool = &pool.kinds[index(manifest.identity.kind)];
    let generation = pool.pending.as_ref().or(pool.active.as_ref()).unwrap();
    let mut refs = vec![weak(&generation.manifest)];
    let server = app.world().resource::<AssetServer>();
    for record in &manifest.artifacts {
        if record.role.starts_with("mesh:") {
            let handle: Handle<Mesh> = server
                .get_handle(
                    GltfAssetLabel::Primitive {
                        mesh: 0,
                        primitive: 0,
                    }
                    .from_asset(record.path.clone()),
                )
                .unwrap();
            refs.push(weak(&handle));
        } else {
            let handle: Handle<Image> = server.get_handle(record.path.clone()).unwrap();
            refs.push(weak(&handle));
        }
    }
    refs
}

#[test]
fn real_glb_and_png_resolve_to_resident_mesh_and_image() {
    let dir = Dir::default();
    let manifest = manifest(BuildingAssetKind::Tank, 1);
    let path = provision(&dir, &manifest, None);
    let mut app = app(&dir);
    let mut pool = BuildingAssetPool::default();
    request(&app, &mut pool, &manifest, &path);
    settle(&mut app, &mut pool, manifest.identity.kind);
    assert_eq!(
        pool.active(manifest.identity.kind),
        Some(&manifest.identity)
    );
    assert!(pool.failure(manifest.identity.kind).is_none());
    let server = app.world().resource::<AssetServer>();
    let mesh: Handle<Mesh> = server
        .get_handle(
            GltfAssetLabel::Primitive {
                mesh: 0,
                primitive: 0,
            }
            .from_asset(manifest.artifacts[0].path.clone()),
        )
        .unwrap();
    assert_eq!(
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .unwrap()
            .count_vertices(),
        3
    );
}

#[test]
fn removed_active_manifest_revokes_the_published_generation() {
    let dir = Dir::default();
    let manifest = manifest(BuildingAssetKind::Tank, 1);
    let path = provision(&dir, &manifest, None);
    let mut app = app(&dir);
    let mut pool = BuildingAssetPool::default();
    request(&app, &mut pool, &manifest, &path);
    settle(&mut app, &mut pool, manifest.identity.kind);

    let handle = pool.kinds[index(manifest.identity.kind)]
        .active
        .as_ref()
        .unwrap()
        .manifest
        .clone();
    app.world_mut()
        .resource_mut::<Assets<BuildingAssetSetManifest>>()
        .remove(handle.id())
        .unwrap();
    assert!(matches!(
        app.world()
            .resource::<AssetServer>()
            .load_state(handle.id()),
        LoadState::Loaded
    ));

    poll(&app, &mut pool);

    assert!(pool.active(manifest.identity.kind).is_none());
    assert!(pool.descriptor(manifest.identity.kind).is_none());
}

#[test]
fn correct_hashes_do_not_make_wrong_glb_or_png_types_ready() {
    for (role, bytes) in [("mesh:body", PNG.to_vec()), ("image:catalog", glb())] {
        let dir = Dir::default();
        let mut manifest = manifest(BuildingAssetKind::Tank, 1);
        let record = manifest
            .artifacts
            .iter_mut()
            .find(|r| r.role == role)
            .unwrap();
        *record = artifact(
            &manifest.identity,
            role,
            if role.starts_with("mesh:") {
                "glb"
            } else {
                "png"
            },
            &bytes,
        );
        seal(&mut manifest);
        let path = provision(&dir, &manifest, Some((role, &bytes)));
        let mut app = app(&dir);
        let mut pool = BuildingAssetPool::default();
        // Byte validation still succeeds: typed decoding has its own failure boundary.
        let raw: Handle<BuildingAssetSetManifest> =
            app.world().resource::<AssetServer>().load(path.clone());
        until(&mut app, |app| {
            app.world()
                .resource::<AssetServer>()
                .is_loaded_with_dependencies(raw.id())
        });
        request(&app, &mut pool, &manifest, &path);
        settle(&mut app, &mut pool, manifest.identity.kind);
        assert!(pool.active(manifest.identity.kind).is_none());
        assert!(pool.failure(manifest.identity.kind).is_some());
    }
}

#[test]
fn world_and_catalog_preview_dimensions_must_match_decoded_png() {
    for catalog in [false, true] {
        let dir = Dir::default();
        let mut manifest = manifest(BuildingAssetKind::Tank, 1);
        let preview = if catalog {
            &mut manifest.catalog_preview
        } else {
            &mut manifest.world_preview
        };
        preview.canvas_px = [2, 2];
        preview.anchor_px = [1., 1.];
        seal(&mut manifest);
        let path = provision(&dir, &manifest, None);
        let mut app = app(&dir);
        let mut pool = BuildingAssetPool::default();
        request(&app, &mut pool, &manifest, &path);
        settle(&mut app, &mut pool, manifest.identity.kind);
        assert!(pool.active(manifest.identity.kind).is_none());
        assert_eq!(
            pool.failure(manifest.identity.kind).unwrap().reason,
            "preview image dimensions differ"
        );
    }
}

#[test]
fn cold_failure_is_latched_and_identity_mismatch_never_requests_typed_roles() {
    let dir = Dir::default();
    let manifest = manifest(BuildingAssetKind::Tank, 1);
    let mut app = app(&dir);
    let mut pool = BuildingAssetPool::default();
    request(&app, &mut pool, &manifest, "missing.buildingset");
    settle(&mut app, &mut pool, manifest.identity.kind);
    assert!(pool.active(manifest.identity.kind).is_none());
    assert!(pool.failure(manifest.identity.kind).is_some());
    for _ in 0..8 {
        assert_eq!(
            pool.request(
                manifest.identity.clone(),
                "missing.buildingset",
                app.world().resource()
            ),
            BuildingAssetRequest::AlreadyAttempted
        );
        poll(&app, &mut pool);
    }
    let path = provision(&dir, &manifest, None);
    let mut wrong = manifest.identity.clone();
    wrong.generation = 2;
    assert_eq!(
        pool.request(wrong, &path, app.world().resource()),
        BuildingAssetRequest::Started
    );
    settle(&mut app, &mut pool, manifest.identity.kind);
    assert_eq!(
        pool.failure(manifest.identity.kind).unwrap().reason,
        "loaded manifest identity differs"
    );
    assert!(app.world().resource::<Assets<Mesh>>().is_empty());
}

#[test]
fn late_residency_keeps_active_until_atomic_switch_and_releases_retired_handles() {
    let dir = Dir::default();
    let first = manifest(BuildingAssetKind::Tank, 1);
    let second = manifest(BuildingAssetKind::Tank, 2);
    let first_path = provision(&dir, &first, None);
    let second_path = provision(&dir, &second, None);
    let mut app = app(&dir);
    let mut pool = BuildingAssetPool::default();
    request(&app, &mut pool, &first, &first_path);
    settle(&mut app, &mut pool, first.identity.kind);
    let retired = references(&app, &pool, &first);
    request(&app, &mut pool, &second, &second_path);
    // Start typed requests, but do not poll the pool once those requests exist.
    until(&mut app, |app| {
        poll(app, &mut pool);
        pool.kinds[0]
            .pending
            .as_ref()
            .is_some_and(|set| set.roles.is_some())
    });
    let catalog: Handle<Image> = app
        .world()
        .resource::<AssetServer>()
        .get_handle(second.artifacts.last().unwrap().path.clone())
        .unwrap();
    until(&mut app, |app| {
        let world = app.world();
        pool.kinds[0]
            .pending
            .as_ref()
            .unwrap()
            .roles
            .as_ref()
            .unwrap()
            .ready(world.resource(), world.resource(), world.resource())
            .unwrap()
    });
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .remove(catalog.id())
        .unwrap();
    let mesh_handle: Handle<Mesh> = app
        .world()
        .resource::<AssetServer>()
        .get_handle(
            GltfAssetLabel::Primitive {
                mesh: 0,
                primitive: 0,
            }
            .from_asset(second.artifacts[0].path.clone()),
        )
        .unwrap();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .remove(mesh_handle.id())
        .unwrap();
    assert!(matches!(
        app.world()
            .resource::<AssetServer>()
            .load_state(catalog.id()),
        LoadState::Loaded
    ));
    for _ in 0..4 {
        poll(&app, &mut pool);
        assert_eq!(pool.active(first.identity.kind), Some(&first.identity));
        assert_eq!(pool.pending(first.identity.kind), Some(&second.identity));
    }
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(catalog.id(), image)
        .unwrap();
    poll(&app, &mut pool);
    assert_eq!(pool.active(first.identity.kind), Some(&first.identity));
    app.world_mut()
        .resource_mut::<Assets<Mesh>>()
        .insert(mesh_handle.id(), mesh)
        .unwrap();
    let owned_counts: Vec<_> = retired.iter().map(Weak::strong_count).collect();
    poll(&app, &mut pool);
    assert_eq!(pool.active(first.identity.kind), Some(&second.identity));
    assert!(pool.pending(first.identity.kind).is_none());
    for (handle, before) in retired.iter().zip(owned_counts) {
        assert!(
            handle.strong_count() < before,
            "pool must drop its references at the switch"
        );
    }
    // Observe AssetServer reclamation separately from the pool's immediate drop.
    until(&mut app, |_| {
        retired.iter().all(|handle| handle.strong_count() == 0)
    });
}

#[test]
fn pending_failure_preserves_active_and_active_revocation_preserves_other_kinds() {
    let dir = Dir::default();
    let first = manifest(BuildingAssetKind::Tank, 1);
    let other = manifest(BuildingAssetKind::OutdoorLamp, 1);
    let mut bad = manifest(BuildingAssetKind::Tank, 2);
    bad.world_preview.canvas_px = [2, 2];
    seal(&mut bad);
    let first_path = provision(&dir, &first, None);
    let other_path = provision(&dir, &other, None);
    let bad_path = provision(&dir, &bad, None);
    let mut app = app(&dir);
    let mut pool = BuildingAssetPool::default();
    request(&app, &mut pool, &first, &first_path);
    request(&app, &mut pool, &other, &other_path);
    settle(&mut app, &mut pool, first.identity.kind);
    settle(&mut app, &mut pool, other.identity.kind);
    request(&app, &mut pool, &bad, &bad_path);
    until(&mut app, |app| {
        poll(app, &mut pool);
        pool.kinds[0]
            .pending
            .as_ref()
            .is_some_and(|set| set.roles.is_some())
    });
    let failed = references(&app, &pool, &bad);
    settle(&mut app, &mut pool, bad.identity.kind);
    assert_eq!(pool.active(first.identity.kind), Some(&first.identity));
    assert_eq!(
        pool.failure(first.identity.kind).unwrap().identity,
        bad.identity
    );
    until(&mut app, |_| {
        failed.iter().all(|handle| handle.strong_count() == 0)
    });
    assert_eq!(
        pool.request(bad.identity.clone(), &bad_path, app.world().resource()),
        BuildingAssetRequest::AlreadyAttempted
    );
    let active_refs = references(&app, &pool, &first);
    assert!(!pool.invalidate_active(&bad.identity));
    assert!(pool.invalidate_active(&first.identity));
    assert!(pool.active(first.identity.kind).is_none());
    assert_eq!(pool.active(other.identity.kind), Some(&other.identity));
    until(&mut app, |_| {
        active_refs.iter().all(|handle| handle.strong_count() == 0)
    });
}

#[test]
fn repeated_generations_keep_pool_bounded_and_invalidation_keeps_pending() {
    let dir = Dir::default();
    let mut app = app(&dir);
    let mut pool = BuildingAssetPool::default();
    for generation in 1..=5 {
        for kind in BuildingAssetKind::ALL {
            let next = manifest(kind, generation);
            let path = provision(&dir, &next, None);
            request(&app, &mut pool, &next, &path);
            let mut third = next.identity.clone();
            third.generation += 1;
            assert_eq!(
                pool.request(third, "not-requested.buildingset", app.world().resource()),
                BuildingAssetRequest::PendingOccupied
            );
            if generation > 1 {
                let active = pool.active(kind).unwrap().clone();
                assert!(pool.invalidate_active(&active));
                assert_eq!(pool.pending(kind), Some(&next.identity));
            }
        }
        assert_eq!(
            pool.kinds
                .iter()
                .filter(|kind| kind.pending.is_some())
                .count(),
            8
        );
        for kind in BuildingAssetKind::ALL {
            settle(&mut app, &mut pool, kind);
            assert_eq!(pool.active(kind).unwrap().generation, generation);
            assert!(pool.pending(kind).is_none());
        }
        assert_eq!(
            pool.kinds
                .iter()
                .filter(|kind| kind.active.is_some())
                .count(),
            8
        );
    }
}
