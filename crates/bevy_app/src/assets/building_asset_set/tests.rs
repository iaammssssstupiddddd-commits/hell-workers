use std::{path::Path, thread, time::Duration};

use bevy::asset::{
    AssetMetaCheck, LoadState,
    io::{
        AssetSourceBuilder, AssetSourceId,
        memory::{Dir, MemoryAssetReader},
    },
};
use bevy::prelude::*;

use super::validation::{canonical, digest, manifest_digest, validate_payload, validate_receipt};
use super::*;

fn artifact(
    identity: &BuildingAssetSetIdentity,
    role: &str,
    extension: &str,
    bytes: &[u8],
) -> BuildingArtifact {
    let sha256 = digest(bytes);
    BuildingArtifact {
        role: role.into(),
        path: format!(
            "building_sets/{}/{}/{sha256}.{extension}",
            identity.kind.slug(),
            identity.generation
        ),
        bytes: bytes.len() as u64,
        sha256,
    }
}

fn fixture(kind: BuildingAssetKind, authority: BuildingAssetAuthority) -> BuildingAssetSetManifest {
    let identity = BuildingAssetSetIdentity {
        kind,
        generation: 1,
        authority,
        manifest_sha256: String::new(),
    };
    let artifacts = kind
        .mesh_roles()
        .iter()
        .map(|r| artifact(&identity, &format!("mesh:{r}"), "glb", b"payload"))
        .chain(
            kind.image_roles()
                .iter()
                .map(|r| artifact(&identity, &format!("image:{r}"), "png", b"payload")),
        )
        .collect();
    let parts = kind
        .mesh_roles()
        .iter()
        .flat_map(|role| {
            let count = if *role == "slot" { 4 } else { 1 };
            (0..count).map(move |index| BuildingAssetPart {
                name: if *role == "slot" {
                    format!("slot{index}")
                } else {
                    role.to_string()
                },
                mesh_role: role.to_string(),
                material_role: if *role == "slot" {
                    BuildingPartMaterial::SpaSlot
                } else {
                    BuildingPartMaterial::OpaqueAlbedo
                },
                translation_wu: [0.0; 3],
                rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            })
        })
        .collect();
    let preview = |image_role: &str| BuildingAssetPreview {
        image_role: image_role.into(),
        canvas_px: [256, 256],
        canvas_wu: [64.0, 64.0],
        anchor_px: [128.0, 128.0],
        representative_state: kind.representative_state().into(),
    };
    let mut manifest = BuildingAssetSetManifest {
        schema_version: 1,
        asset_set_id: format!("building-{}-v1", kind.slug()),
        identity,
        source_sha256: "a".repeat(64),
        export_sha256: "b".repeat(64),
        geometry_contract_sha256: "c".repeat(64),
        art_approval_sha256: (authority != BuildingAssetAuthority::ArtPreview)
            .then(|| "d".repeat(64)),
        artifacts,
        parts,
        world_preview: preview(kind.world_preview_role()),
        catalog_preview: preview("catalog"),
        receipt: None,
    };
    seal(&mut manifest);
    manifest
}

fn receipt_bytes(manifest: &BuildingAssetSetManifest) -> Vec<u8> {
    canonical(&BuildingPromotionReceipt {
        schema_version: 1,
        identity: manifest.identity.clone(),
        art_approval_sha256: manifest.art_approval_sha256.clone().unwrap_or_default(),
        decision: "release_approved".into(),
    })
    .unwrap()
}

fn seal(manifest: &mut BuildingAssetSetManifest) {
    manifest.identity.manifest_sha256 = manifest_digest(manifest).unwrap();
    if manifest.identity.authority == BuildingAssetAuthority::ReleaseApproved {
        manifest.receipt = Some(artifact(
            &manifest.identity,
            "authority:receipt",
            "json",
            &receipt_bytes(manifest),
        ));
    }
}

fn rejects(mut manifest: BuildingAssetSetManifest) {
    seal(&mut manifest);
    assert!(decode_buildingset(&canonical(&manifest).unwrap()).is_err());
}

#[test]
fn all_eight_kind_inventories_match_the_authoring_contract() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/blender_ai_workflow/fixtures/building-art-v1.contract.json"
    )))
    .unwrap();
    for kind in BuildingAssetKind::ALL {
        let name = serde_json::to_value(kind).unwrap();
        let expected = &contract["kinds"][name.as_str().unwrap()];
        assert_eq!(
            expected["mesh_roles"],
            serde_json::to_value(kind.mesh_roles()).unwrap()
        );
        assert_eq!(
            expected["image_roles"],
            serde_json::to_value(kind.image_roles()).unwrap()
        );
        assert_eq!(
            expected["representative_state"],
            kind.representative_state()
        );
        for authority in [
            BuildingAssetAuthority::ArtPreview,
            BuildingAssetAuthority::IsolatedCandidate,
            BuildingAssetAuthority::ReleaseApproved,
        ] {
            let manifest = fixture(kind, authority);
            assert_eq!(
                expected["leaf_count"].as_u64().unwrap(),
                manifest.parts.len() as u64
            );
            assert_eq!(
                decode_buildingset(&canonical(&manifest).unwrap()).unwrap(),
                manifest
            );
        }
    }
    for excluded in ["Bridge", "Wall", "Floor", "Door", "Unknown"] {
        assert!(serde_json::from_value::<BuildingAssetKind>(serde_json::json!(excluded)).is_err());
    }
}

#[test]
fn canonical_json_content_hash_and_unknown_fields_are_enforced() {
    let manifest = fixture(BuildingAssetKind::Tank, BuildingAssetAuthority::ArtPreview);
    assert!(decode_buildingset(&serde_json::to_vec_pretty(&manifest).unwrap()).is_err());
    let mut stale = manifest.clone();
    stale.parts[0].translation_wu[0] = 4.0;
    assert!(decode_buildingset(&canonical(&stale).unwrap()).is_err());
    let mut value = serde_json::to_value(&manifest).unwrap();
    value["unknown"] = true.into();
    assert!(serde_json::from_value::<BuildingAssetSetManifest>(value).is_err());
    let mut invalid = manifest;
    invalid.identity.generation = 0;
    rejects(invalid);
}

#[test]
fn role_inventory_and_content_address_reject_aliases_and_traversal() {
    let manifest = fixture(BuildingAssetKind::Tank, BuildingAssetAuthority::ArtPreview);
    let mut missing = manifest.clone();
    missing.artifacts.pop();
    rejects(missing);
    let mut duplicate = manifest.clone();
    duplicate.artifacts[1] = duplicate.artifacts[0].clone();
    rejects(duplicate);
    let mut reordered = manifest.clone();
    reordered.artifacts.swap(0, 1);
    rejects(reordered);
    for path in [
        "../other.png",
        "/tmp/other.png",
        "https://example.invalid/other.png",
        "other://file.png",
        "building_sets/tank/2/other.png",
        "file.glb#Mesh0/Primitive0",
    ] {
        let mut invalid = manifest.clone();
        invalid.artifacts[0].path = path.into();
        rejects(invalid);
    }
    let mut zero_bytes = manifest.clone();
    zero_bytes.artifacts[0].bytes = 0;
    rejects(zero_bytes);
    let mut wrong_kind = manifest;
    wrong_kind.asset_set_id = "building-rest-area-v1".into();
    rejects(wrong_kind);
}

#[test]
fn transforms_previews_and_shared_spa_slots_have_exact_contracts() {
    let manifest = fixture(
        BuildingAssetKind::SoulSpa,
        BuildingAssetAuthority::ArtPreview,
    );
    let mut missing = manifest.clone();
    missing.parts.pop();
    rejects(missing);
    let mut duplicated = manifest.clone();
    duplicated.parts[4].name = "slot0".into();
    rejects(duplicated);
    let mut wrong_mesh = manifest.clone();
    wrong_mesh.parts[1].mesh_role = "body".into();
    rejects(wrong_mesh);
    let mut wrong_material = manifest.clone();
    wrong_material.parts[1].material_role = BuildingPartMaterial::OpaqueAlbedo;
    rejects(wrong_material);
    for scale in [0.0, -1.0, f32::INFINITY] {
        let mut invalid = manifest.clone();
        invalid.parts[0].scale[0] = scale;
        rejects(invalid);
    }
    let mut nan = manifest.clone();
    nan.parts[0].translation_wu[0] = f32::NAN;
    rejects(nan);
    let mut quaternion = manifest.clone();
    quaternion.parts[0].rotation_xyzw = [0.0; 4];
    rejects(quaternion);
    let mut canvas = manifest.clone();
    canvas.world_preview.canvas_px[0] = 0;
    rejects(canvas);
    let mut anchor = manifest.clone();
    anchor.world_preview.anchor_px[0] = 257.0;
    rejects(anchor);
    let mut size = manifest.clone();
    size.world_preview.canvas_wu[0] = -1.0;
    rejects(size);
    let mut state = manifest.clone();
    state.world_preview.representative_state = "Constructing".into();
    rejects(state);
    let mut catalog = manifest.clone();
    catalog.catalog_preview.anchor_px[1] = 200.0;
    rejects(catalog);
    let mut catalog = manifest;
    catalog.catalog_preview.canvas_px[0] = 128;
    rejects(catalog);
}

#[test]
fn candidate_policy_requires_exact_kind_generation_authority_and_hash() {
    let candidate = fixture(
        BuildingAssetKind::Tank,
        BuildingAssetAuthority::IsolatedCandidate,
    );
    assert!(!BuildingAssetLoadPolicy::default().allows(&candidate.identity));
    let policy = BuildingAssetLoadPolicy {
        allowed_candidate: Some(candidate.identity.clone()),
    };
    assert!(policy.allows(&candidate.identity));
    let mut wrong = candidate.identity.clone();
    wrong.generation += 1;
    assert!(!policy.allows(&wrong));
    let mut wrong = candidate.identity.clone();
    wrong.kind = BuildingAssetKind::MudMixer;
    assert!(!policy.allows(&wrong));
    let mut wrong = candidate.identity.clone();
    wrong.manifest_sha256 = "e".repeat(64);
    assert!(!policy.allows(&wrong));
    let mut preview = candidate.identity;
    preview.authority = BuildingAssetAuthority::ArtPreview;
    assert!(!policy.allows(&preview));
    let preview_policy = BuildingAssetLoadPolicy {
        allowed_candidate: Some(preview.clone()),
    };
    assert_eq!(preview_policy.allows(&preview), cfg!(feature = "profiling"));
}

#[test]
fn receipt_binds_manifest_approval_and_payload_bytes() {
    let manifest = fixture(
        BuildingAssetKind::Tank,
        BuildingAssetAuthority::ReleaseApproved,
    );
    let bytes = receipt_bytes(&manifest);
    validate_receipt(&manifest, &bytes).unwrap();
    validate_payload(manifest.receipt.as_ref().unwrap(), &bytes).unwrap();
    assert!(validate_payload(manifest.receipt.as_ref().unwrap(), b"tampered").is_err());
    let mut receipt: BuildingPromotionReceipt = serde_json::from_slice(&bytes).unwrap();
    receipt.identity.generation += 1;
    assert!(validate_receipt(&manifest, &canonical(&receipt).unwrap()).is_err());
    receipt.identity = manifest.identity.clone();
    receipt.art_approval_sha256 = "e".repeat(64);
    assert!(validate_receipt(&manifest, &canonical(&receipt).unwrap()).is_err());
    let mut candidate = fixture(
        BuildingAssetKind::Tank,
        BuildingAssetAuthority::IsolatedCandidate,
    );
    candidate.receipt = manifest.receipt.clone();
    rejects(candidate);
    let mut preview = fixture(BuildingAssetKind::Tank, BuildingAssetAuthority::ArtPreview);
    preview.art_approval_sha256 = Some("a".repeat(64));
    rejects(preview);
    let mut missing = manifest;
    missing.receipt = None;
    assert!(decode_buildingset(&canonical(&missing).unwrap()).is_err());
}

fn asset_app(dir: &Dir, policy: BuildingAssetLoadPolicy) -> App {
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
    ))
    .insert_resource(policy)
    .init_asset::<BuildingAssetSetManifest>()
    .init_asset_loader::<BuildingAssetSetLoader>();
    app.finish();
    app.cleanup();
    app
}

fn load(app: &mut App, path: &str) -> LoadState {
    let handle: Handle<BuildingAssetSetManifest> =
        app.world().resource::<AssetServer>().load(path.to_owned());
    for _ in 0..1000 {
        app.update();
        let state = app
            .world()
            .resource::<AssetServer>()
            .load_state(handle.id());
        if matches!(state, LoadState::Loaded | LoadState::Failed(_)) {
            return state;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("buildingset did not reach a terminal load state");
}

fn provision(dir: &Dir, path: &str, manifest: &BuildingAssetSetManifest) {
    dir.insert_asset(Path::new(path), canonical(manifest).unwrap());
    for record in &manifest.artifacts {
        dir.insert_asset(Path::new(&record.path), b"payload".to_vec());
    }
    if let Some(receipt) = &manifest.receipt {
        dir.insert_asset(Path::new(&receipt.path), receipt_bytes(manifest));
    }
}

#[test]
fn real_asset_server_checks_all_bytes_and_rejects_missing_or_corrupt_dependencies() {
    for failure in [None, Some("missing"), Some("corrupt"), Some("receipt")] {
        let dir = Dir::default();
        let manifest = fixture(
            BuildingAssetKind::Tank,
            BuildingAssetAuthority::ReleaseApproved,
        );
        provision(&dir, "tank.buildingset", &manifest);
        match failure {
            Some("missing") => {
                dir.remove_asset(Path::new(&manifest.artifacts[0].path));
            }
            Some("corrupt") => {
                dir.insert_asset(Path::new(&manifest.artifacts[0].path), b"wrong".to_vec())
            }
            Some("receipt") => dir.insert_asset(
                Path::new(&manifest.receipt.as_ref().unwrap().path),
                b"wrong".to_vec(),
            ),
            _ => {}
        }
        let mut app = asset_app(&dir, default());
        assert_eq!(
            matches!(load(&mut app, "tank.buildingset"), LoadState::Loaded),
            failure.is_none()
        );
    }
}

#[test]
fn unauthorized_candidate_stops_before_dependency_io_and_policy_is_snapshotted() {
    let dir = Dir::default();
    let manifest = fixture(
        BuildingAssetKind::Tank,
        BuildingAssetAuthority::IsolatedCandidate,
    );
    dir.insert_asset(
        Path::new("candidate.buildingset"),
        canonical(&manifest).unwrap(),
    );
    let mut app = asset_app(&dir, default());
    app.world_mut()
        .resource_mut::<BuildingAssetLoadPolicy>()
        .allowed_candidate = Some(manifest.identity.clone());
    let LoadState::Failed(error) = load(&mut app, "candidate.buildingset") else {
        panic!("unauthorized candidate loaded");
    };
    assert!(
        error.to_string().contains("authority is not enabled"),
        "{error}"
    );
    provision(&dir, "candidate.buildingset", &manifest);
    let mut authorized = asset_app(
        &dir,
        BuildingAssetLoadPolicy {
            allowed_candidate: Some(manifest.identity),
        },
    );
    assert!(matches!(
        load(&mut authorized, "candidate.buildingset"),
        LoadState::Loaded
    ));
}
