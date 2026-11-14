//! Closed paint-projection experiment. It cannot change normal asset authority.
use super::super::gallery_capture::GalleryCaptureSession;
use super::super::validation::digest;
use super::gallery::ReleaseArtSession;
use super::*;
use crate::plugins::startup::{PerfScenarioConfig, PerfScenarioSize, PerfWorkload};
use hw_core::quality::RttQualityPreset;
use std::path::PathBuf;

pub(crate) const ENV: &str = "HW_BUILDING_TANK_ILLUSTRATION_PILOT";
pub(crate) const CONTRACT: &str = "building-tank-illustration-pilot-v1";
pub(crate) const PAINT_PATH: &str = "pilot/tank-body.png";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PaintProjection {
    pub path: String,
    pub png_sha256: String,
    pub rgba_sha256: String,
    pub dimensions: [u32; 2],
    pub canvas_wu: [f32; 2],
    pub anchor_uv: [f32; 2],
    pub matte_linear_rgb: [f32; 3],
    pub body_glb_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TankIllustrationPilot {
    pub schema_version: u8,
    pub contract_id: String,
    pub gallery: ReleaseArtSession,
    pub paint: PaintProjection,
}

fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

impl TankIllustrationPilot {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        let value: Self = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        ReleaseArtSession::parse(
            &serde_json::to_string(&value.gallery).map_err(|e| e.to_string())?,
        )?;
        let p = &value.paint;
        if value.schema_version != 1
            || value.contract_id != CONTRACT
            || p.path != PAINT_PATH
            || !digest_valid(&p.png_sha256)
            || !digest_valid(&p.rgba_sha256)
            || p.body_glb_sha256
                != "d14c32ac93d23db22d3158b0b02b202377d4e48c9326e26b18d8c6a001f2cf28"
            || p.dimensions.iter().any(|n| *n == 0 || *n > 2048)
            || p.canvas_wu != [96.0, 96.0]
            || p.anchor_uv != [0.5, 0.625]
            || p.matte_linear_rgb
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || value
                .gallery
                .bindings
                .iter()
                .find(|b| b.identity.kind == BuildingAssetKind::Tank)
                .is_none_or(|b| {
                    b.identity.generation != 3
                        || b.identity.manifest_sha256
                            != "3eb6285f60f837280e3d97604ce055e0c651cfa80d2dd95c0937c86f27fd4ba2"
                })
        {
            return Err("invalid exact Tank illustration pilot inputs".into());
        }
        Ok(value)
    }

    pub(crate) fn window_matches(
        &self,
        width: Option<u32>,
        height: Option<u32>,
        dpi: Option<f32>,
        quality: Option<RttQualityPreset>,
    ) -> bool {
        self.gallery.window_matches(width, height, dpi, quality)
    }
}

#[derive(Resource, Clone)]
pub(crate) struct TankPilotPaint {
    pub session: TankIllustrationPilot,
    pub image: Handle<Image>,
}

impl TankPilotPaint {
    pub(crate) fn matches_identity(&self, identity: &BuildingAssetSetIdentity) -> bool {
        self.session
            .gallery
            .bindings
            .iter()
            .any(|b| b.identity.kind == BuildingAssetKind::Tank && &b.identity == identity)
    }

    pub(crate) fn body_material_matches(
        &self,
        material: &hw_visual::TopDownStructuralMaterial,
    ) -> bool {
        let p = &self.session.paint;
        material.base.base_color_texture.as_ref() == Some(&self.image)
            && material.base.base_color == Color::WHITE
            && !material.base.unlit
            && material.base.alpha_mode == AlphaMode::Opaque
            && material.base.emissive == LinearRgba::BLACK
            && material.base.emissive_texture.is_none()
            && material.base.normal_map_texture.is_none()
            && material.base.metallic_roughness_texture.is_none()
            && material.base.occlusion_texture.is_none()
            && material.base.perceptual_roughness == 1.0
            && material.base.reflectance == 0.0
            && material.base.metallic == 0.0
            && material.extension.uniforms.indoor_light_params.w == 2.0
            && material.extension.uniforms.shadow_style_params
                == Vec4::new(
                    p.canvas_wu[0],
                    p.canvas_wu[1],
                    p.anchor_uv[0],
                    p.anchor_uv[1],
                )
            && material.extension.uniforms.shadow_style_tint
                == Vec4::new(
                    p.matte_linear_rgb[0],
                    p.matte_linear_rgb[1],
                    p.matte_linear_rgb[2],
                    1.0,
                )
    }

    pub(crate) fn ready(&self, images: &Assets<Image>) -> Result<bool, String> {
        let Some(image) = images.get(&self.image) else {
            return Ok(false);
        };
        let p = &self.session.paint;
        if image.texture_descriptor.size.width != p.dimensions[0]
            || image.texture_descriptor.size.height != p.dimensions[1]
            || image.texture_descriptor.format
                != bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
            || image
                .data
                .as_ref()
                .is_none_or(|data| digest(data) != p.rgba_sha256)
        {
            return Err("pilot decoded image differs from the pinned paint".into());
        }
        Ok(true)
    }
}

pub(super) fn configure(app: &mut App) -> Result<(), String> {
    let session = TankIllustrationPilot::parse(&std::env::var(ENV).map_err(|e| e.to_string())?)?;
    if [
        "HW_BUILDING_RELEASE_GALLERY_SESSION",
        "HW_BUILDING_GROUP_ART_SESSION",
        "HW_BUILDING_GROUP_COMPARISON",
        "HW_BUILDING_M6_MODE",
        "HW_BUILDING_M6_CANDIDATES",
        "HW_BUILDING_ASSET_RELEASES",
        "HW_BUILDING_ART_SESSION",
        "HW_BUILDING_ORDINARY_ART_SESSION",
        "HW_BUILDING_PRODUCTION_OBSERVATIONS",
        "HW_M2_ACCEPTANCE_PROBE",
        "HW_BRIDGE_ACCEPTANCE_PROBE",
        "HW_NATIVE_UI_ROOT",
    ]
    .into_iter()
    .any(|k| std::env::var_os(k).is_some())
    {
        return Err("Tank pilot cannot mix with another session".into());
    }
    let config = PerfScenarioConfig::try_from_process().map_err(|e| e.to_string())?;
    let directory = session
        .gallery
        .status_path
        .parent()
        .ok_or("missing pilot root")?;
    let native_root = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("target/native-acceptance");
    if !config.enabled()
        || config.workload() != PerfWorkload::BuildingArtStatic
        || config.size() != PerfScenarioSize::Small
        || config.output_dir() != Some(directory.join("data").as_path())
        || !session.window_matches(
            config.requested_window_size().map(|x| x.0),
            config.requested_window_size().map(|x| x.1),
            config.requested_window_scale_factor(),
            config.requested_rtt_quality(),
        )
        || !directory.starts_with(native_root)
        || directory.canonicalize().map_err(|e| e.to_string())? != directory
        || session.gallery.status_path.exists()
    {
        return Err("pilot requires its exact fresh native session".into());
    }
    let asset_root = PathBuf::from(std::env::var("BEVY_ASSET_ROOT").map_err(|e| e.to_string())?);
    let expected_view = directory
        .parent()
        .and_then(Path::parent)
        .ok_or("missing isolated pilot view")?
        .join("asset-view");
    if asset_root != expected_view
        || asset_root.canonicalize().map_err(|e| e.to_string())? != asset_root
    {
        return Err("pilot cannot use primary or foreign assets".into());
    }
    let paint_path = asset_root.join("assets").join(PAINT_PATH);
    if !paint_path.is_file()
        || paint_path.is_symlink()
        || paint_path.canonicalize().map_err(|e| e.to_string())? != paint_path
    {
        return Err(
            "pilot paint must be a canonical regular file in the isolated asset view".into(),
        );
    }
    let bytes = std::fs::read(&paint_path).map_err(|e| e.to_string())?;
    if digest(&bytes) != session.paint.png_sha256 {
        return Err("pilot PNG digest differs".into());
    }
    let admitted = comparison("candidate", &session.gallery.bindings)?;
    app.init_resource::<BuildingAssetPool>()
        .insert_resource(admitted)
        .insert_resource(BuildingAssetReleaseBindings(
            session.gallery.bindings.clone(),
        ))
        .insert_resource(GalleryCaptureSession::TankPilot(session))
        .add_systems(Startup, (request_releases, request_paint));
    Ok(())
}

fn request_paint(
    mut commands: Commands,
    server: Res<AssetServer>,
    gallery: Res<GalleryCaptureSession>,
) {
    let GalleryCaptureSession::TankPilot(session) = gallery.as_ref() else {
        panic!("Tank pilot startup lost its admitted session");
    };
    commands.insert_resource(TankPilotPaint {
        image: server.load(PAINT_PATH),
        session: session.clone(),
    });
}

fn pose(value: &Transform) -> serde_json::Value {
    serde_json::json!({"translation": value.translation.to_array(), "rotation": value.rotation.to_array(), "scale": value.scale.to_array()})
}

fn image_witness(world: &World, handle: &Handle<Image>) -> Result<serde_json::Value, String> {
    let image = world
        .resource::<Assets<Image>>()
        .get(handle)
        .ok_or("pilot image missing")?;
    let data = image.data.as_ref().ok_or("pilot image has no CPU bytes")?;
    Ok(serde_json::json!({"handle": format!("{:?}", handle.id()),
        "asset_path": world.resource::<AssetServer>().get_path(handle.id()).map(|p| p.path().to_string_lossy().into_owned()),
        "width": image.texture_descriptor.size.width, "height": image.texture_descriptor.size.height,
        "format": format!("{:?}", image.texture_descriptor.format), "mip_levels": image.texture_descriptor.mip_level_count,
        "rgba_sha256": digest(data), "bytes": data.len()}))
}

/// Collect the actual parent edge independently of the material/Children walk.
pub(crate) fn hierarchy(world: &mut World) -> serde_json::Value {
    use hw_jobs::{Building, BuildingType};
    use hw_visual::Building3dVisual;
    let mut edges: Vec<_> = world
        .query::<(Entity, &Mesh3d, &ChildOf)>()
        .iter(world)
        .filter_map(|(entity, _, parent)| {
            let visual = world.get::<Building3dVisual>(parent.parent())?;
            let building = world.get::<Building>(visual.owner)?;
            (building.kind == BuildingType::Tank).then(|| {
                serde_json::json!({"entity": entity.to_bits(), "parent": parent.parent().to_bits(), "owner": visual.owner.to_bits()})
            })
        })
        .collect();
    edges.sort_by_key(|edge| edge["entity"].as_u64().unwrap_or_default());
    serde_json::json!(edges)
}

/// Derive the proof from real ECS/material/image resources, not the declaration.
pub(crate) fn witness(world: &mut World) -> Result<Option<serde_json::Value>, String> {
    use crate::systems::visual::building_presentation::EquipmentRoot;
    use hw_jobs::{Building, BuildingType};
    use hw_visual::{Building3dVisual, TopDownStructuralMaterial};
    let pilot = world
        .get_resource::<TankPilotPaint>()
        .ok_or("pilot paint resource missing")?
        .clone();
    if !pilot.ready(world.resource::<Assets<Image>>())? {
        return Ok(None);
    }
    let set = world
        .resource::<BuildingAssetPool>()
        .descriptor(BuildingAssetKind::Tank)
        .ok_or("pilot Tank descriptor missing")?;
    if !pilot.matches_identity(&set.manifest.identity) {
        return Err("pilot resolved another Tank".into());
    }
    let identity = set.manifest.identity.clone();
    let body = set.mesh("body").ok_or("pilot body missing")?.clone();
    let water = set.mesh("water").ok_or("pilot water missing")?.clone();
    let water_texture = set
        .image("albedo")
        .ok_or("pilot water atlas missing")?
        .clone();
    let roots: Vec<_> = world
        .query::<(Entity, &Building3dVisual, &EquipmentRoot, &Children)>()
        .iter(world)
        .filter(|(_, visual, _, _)| {
            world
                .get::<Building>(visual.owner)
                .is_some_and(|b| b.kind == BuildingType::Tank)
        })
        .map(|(entity, visual, root, children)| {
            (
                entity,
                visual.owner,
                root.observed_identity().cloned(),
                children.iter().collect::<Vec<_>>(),
            )
        })
        .collect();
    if roots.len() != 4 {
        return Err("pilot requires exactly four Tank owners".into());
    }
    let mut rows = Vec::new();
    for (root, owner, observed_identity, children) in roots {
        if observed_identity.as_ref() != Some(&identity) || children.len() != 2 {
            return Err("pilot root identity/parts differ".into());
        }
        let transform = world
            .get::<Transform>(root)
            .ok_or("pilot Root transform missing")?;
        if transform.translation.y != 0.0
            || transform.rotation != Quat::IDENTITY
            || transform.scale != Vec3::ONE
        {
            return Err("pilot requires ground Root at unit scale/yaw".into());
        }
        let mut parts = Vec::new();
        for part in children {
            if world.get::<ChildOf>(part).map(ChildOf::parent) != Some(root) {
                return Err("pilot part actual parent differs".into());
            }
            let mesh = world.get::<Mesh3d>(part).ok_or("pilot part mesh missing")?;
            let role = if mesh.0 == body {
                "body"
            } else if mesh.0 == water {
                "water"
            } else {
                return Err("pilot foreign mesh".into());
            };
            let handle = world
                .get::<MeshMaterial3d<TopDownStructuralMaterial>>(part)
                .ok_or("pilot material handle missing")?;
            let material = world
                .resource::<Assets<TopDownStructuralMaterial>>()
                .get(&handle.0)
                .ok_or("pilot material missing")?;
            if role == "body" {
                if !pilot.body_material_matches(material) {
                    return Err("pilot body material differs".into());
                }
            } else if material.base.base_color_texture.as_ref() != Some(&water_texture)
                || material.extension.uniforms.indoor_light_params.w != 0.82
                || material.base.unlit
            {
                return Err("paint mixed into pilot water".into());
            }
            let local = world
                .get::<Transform>(part)
                .ok_or("pilot local pose missing")?;
            let global = world
                .get::<GlobalTransform>(part)
                .ok_or("pilot global pose missing")?;
            parts.push(serde_json::json!({"entity": part.to_bits(), "role": role, "mesh": format!("{:?}", mesh.id()),
                "material": format!("{:?}", handle.0.id()), "mode": material.extension.uniforms.indoor_light_params.w,
                "unlit": material.base.unlit, "alpha": format!("{:?}", material.base.alpha_mode),
                "base_color": material.base.base_color.to_srgba().to_f32_array(),
                "texture": material.base.base_color_texture.as_ref().map(|h| format!("{:?}", h.id())),
                "projection": material.extension.uniforms.shadow_style_params.to_array(), "matte": material.extension.uniforms.shadow_style_tint.to_array(),
                "local": pose(local), "global": global.to_matrix().to_cols_array(),
                "visible": world.get::<ViewVisibility>(part).is_some_and(|visible| visible.get())}));
        }
        parts.sort_by_key(|p| p["role"].as_str().unwrap_or_default().to_owned());
        rows.push(serde_json::json!({"owner": owner.to_bits(), "root": root.to_bits(), "identity": observed_identity,
            "root_pose": pose(transform), "root_global": world.get::<GlobalTransform>(root).ok_or("pilot root global missing")?.to_matrix().to_cols_array(), "parts": parts}));
    }
    rows.sort_by_key(|r| r["owner"].as_u64().unwrap_or_default());
    Ok(Some(
        serde_json::json!({"profile": CONTRACT, "accepted": false, "promotion_authority": false, "performance_evidence": false,
        "paint_image": image_witness(world, &pilot.image)?, "water_image": image_witness(world, &water_texture)?, "owners": rows}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session_value() -> serde_json::Value {
        let mut gallery = serde_json::to_value(super::super::gallery::test_session()).unwrap();
        gallery["bindings"][0]["identity"]["generation"] = json!(3);
        gallery["bindings"][0]["identity"]["manifest_sha256"] =
            json!("3eb6285f60f837280e3d97604ce055e0c651cfa80d2dd95c0937c86f27fd4ba2");
        json!({"schema_version":1,"contract_id":CONTRACT,"gallery":gallery,"paint":{
            "path":PAINT_PATH,"png_sha256":"a".repeat(64),"rgba_sha256":"b".repeat(64),"dimensions":[256,256],"canvas_wu":[96.0,96.0],"anchor_uv":[0.5,0.625],"matte_linear_rgb":[0.125,0.09375,0.078125],"body_glb_sha256":"d14c32ac93d23db22d3158b0b02b202377d4e48c9326e26b18d8c6a001f2cf28"}})
    }

    #[test]
    fn pilot_parser_pins_kind_generation_geometry_canvas_and_closed_scope() {
        let good = session_value();
        assert!(TankIllustrationPilot::parse(&good.to_string()).is_ok());
        for (field, value) in [
            ("contract_id", json!(gallery::RELEASE_GALLERY_CONTRACT)),
            ("schema_version", json!(true)),
            ("accepted", json!(true)),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            assert!(
                TankIllustrationPilot::parse(&bad.to_string()).is_err(),
                "{field}"
            );
        }
        for (field, value) in [
            ("path", json!("../tank.png")),
            ("dimensions", json!([0, 256])),
            ("canvas_wu", json!([95., 96.])),
            ("anchor_uv", json!([0.5, 1.])),
            ("matte_linear_rgb", json!([-0.1, 0., 0.])),
            ("body_glb_sha256", json!("0".repeat(64))),
            ("png_sha256", json!("A".repeat(64))),
        ] {
            let mut bad = good.clone();
            bad["paint"][field] = value;
            assert!(
                TankIllustrationPilot::parse(&bad.to_string()).is_err(),
                "{field}"
            );
        }
        let mut bad = good.clone();
        bad["gallery"]["bindings"][0]["identity"]["generation"] = json!(2);
        assert!(TankIllustrationPilot::parse(&bad.to_string()).is_err());
        let mut bad = good;
        bad["gallery"]["bindings"][0]["identity"]["kind"] = json!("Bridge");
        assert!(TankIllustrationPilot::parse(&bad.to_string()).is_err());
    }

    #[test]
    fn pilot_material_guard_rejects_water_foreign_texture_mode_and_other_channels() {
        let session = TankIllustrationPilot::parse(&session_value().to_string()).unwrap();
        let paint = TankPilotPaint {
            session,
            image: Handle::default(),
        };
        let mut material =
            hw_visual::make_topdown_structural_material(LinearRgba::WHITE, Handle::default());
        material.base.base_color_texture = Some(paint.image.clone());
        material.extension.uniforms.indoor_light_params.w = 2.;
        let p = &paint.session.paint;
        material.extension.uniforms.shadow_style_params = Vec4::new(
            p.canvas_wu[0],
            p.canvas_wu[1],
            p.anchor_uv[0],
            p.anchor_uv[1],
        );
        material.extension.uniforms.shadow_style_tint = Vec4::new(
            p.matte_linear_rgb[0],
            p.matte_linear_rgb[1],
            p.matte_linear_rgb[2],
            1.,
        );
        assert!(paint.body_material_matches(&material));
        let mut bad = material.clone();
        bad.extension.uniforms.indoor_light_params.w = 0.82;
        assert!(!paint.body_material_matches(&bad));
        let mut bad = material.clone();
        bad.base.base_color_texture = None;
        assert!(!paint.body_material_matches(&bad));
        let mut bad = material.clone();
        bad.base.normal_map_texture = Some(Handle::default());
        assert!(!paint.body_material_matches(&bad));
        let mut bad = material.clone();
        bad.base.unlit = true;
        assert!(!paint.body_material_matches(&bad));
        let mut bad = material;
        bad.extension.uniforms.shadow_style_params.x = 95.;
        assert!(!paint.body_material_matches(&bad));
    }
    #[test]
    fn pilot_paint_is_requested_after_asset_plugin_registration() {
        let session = TankIllustrationPilot::parse(&session_value().to_string()).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(GalleryCaptureSession::TankPilot(session))
            .add_systems(Startup, request_paint);
        assert!(!app.world().contains_resource::<AssetServer>());
        app.add_plugins(AssetPlugin::default())
            .init_asset::<Image>();
        app.update();
        assert!(app.world().contains_resource::<TankPilotPaint>());
        assert_eq!(
            app.world()
                .resource::<AssetServer>()
                .get_path(app.world().resource::<TankPilotPaint>().image.id())
                .unwrap()
                .path(),
            Path::new(PAINT_PATH)
        );
    }

    #[test]
    fn pilot_ready_rejects_missing_changed_or_foreign_decoded_image() {
        let mut session = TankIllustrationPilot::parse(&session_value().to_string()).unwrap();
        let mut image = Image::default();
        image.texture_descriptor.size.width = 2;
        image.texture_descriptor.size.height = 2;
        image.texture_descriptor.format =
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb;
        image.data = Some(vec![123; 16]);
        session.paint.dimensions = [2, 2];
        session.paint.rgba_sha256 = digest(image.data.as_ref().unwrap());
        let mut images = Assets::<Image>::default();
        let handle = images.add(image);
        let paint = TankPilotPaint {
            session,
            image: handle.clone(),
        };
        assert_eq!(paint.ready(&images), Ok(true));
        images.get_mut(&handle).unwrap().data.as_mut().unwrap()[0] = 124;
        assert!(paint.ready(&images).is_err());
        images.remove(handle.id());
        assert_eq!(paint.ready(&images), Ok(false));
    }
}
