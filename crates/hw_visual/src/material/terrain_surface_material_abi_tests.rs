use super::*;
const TYPES: &str = include_str!("../../../../assets/shaders/terrain_surface_types.wgsl");
const BINDINGS: &str = include_str!("../../../../assets/shaders/terrain_surface_bindings.wgsl");
const EXPECTED_FIELDS: [(&str, &str); 14] = [
    ("map_world_width", "f32"),
    ("map_world_height", "f32"),
    ("uv_scale", "f32"),
    ("blend_strength", "f32"),
    ("macro_noise_scale", "f32"),
    ("overlay_scale", "f32"),
    ("lut_shore", "vec4<f32>"),
    ("lut_inland", "vec4<f32>"),
    ("lut_rock", "vec4<f32>"),
    ("feature_lut_constants_ready", "f32"),
    ("shadow_style_params", "vec4<f32>"),
    ("shadow_style_tint", "vec4<f32>"),
    ("shadow_style_blur", "vec4<f32>"),
    ("indoor_light_params", "vec4<f32>"),
];
fn fields(source: &str) -> Vec<(&str, &str)> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.split("//").next()?.trim().trim_end_matches(',');
            let (name, ty) = line.split_once(':')?;
            Some((name.trim(), ty.trim()))
        })
        .collect()
}
fn bindings(source: &str) -> Option<Vec<(u32, &str, &str)>> {
    source
        .lines()
        .filter(|line| line.contains("@binding("))
        .map(|line| {
            let line = line
                .trim()
                .strip_prefix("@group(#{MATERIAL_BIND_GROUP}) ")?;
            let (_, after) = line.split_once("@binding(")?;
            let (number, variable) = after.split_once(')')?;
            let (qualifier, declaration) = variable.trim().split_once(' ')?;
            if qualifier
                != if number == "100" {
                    "var<uniform>"
                } else {
                    "var"
                }
            {
                return None;
            }
            let (name, ty) = declaration.trim().trim_end_matches(';').split_once(':')?;
            Some((number.parse().ok()?, name.trim(), ty.trim()))
        })
        .collect()
}

fn rust_struct<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source.find(&format!("pub struct {name} {{")).unwrap();
    &source[start..][..source[start..].find("\n}").unwrap()]
}

fn rust_bindings(source: &str) -> Option<Vec<(u32, &str, &str)>> {
    let mut pending = Vec::new();
    let mut output = Vec::new();
    for line in source.lines().map(str::trim) {
        if let Some(attribute) = line.strip_prefix("#[") {
            let (kind, slot) = attribute.strip_suffix(")]")?.split_once('(')?;
            pending.push((slot.parse().ok()?, kind));
        } else if let Some(field) = line
            .strip_prefix("pub ")
            .and_then(|line| line.split_once(':'))
        {
            for (slot, kind) in pending.drain(..) {
                output.push((slot, kind, field.0));
            }
        }
    }
    pending.is_empty().then_some(output)
}
#[test]
fn terrain_uniform_abi_matches_shared_wgsl_test() {
    assert_eq!(fields(TYPES), EXPECTED_FIELDS);
    let rust_source = include_str!("terrain_surface_material.rs");
    let rust_fields = |source: &str| {
        fields(rust_struct(source, "TerrainSurfaceUniform"))
            .into_iter()
            .map(|(name, ty)| {
                (
                    name.trim_start_matches("pub ").to_string(),
                    if ty == "Vec4" { "vec4<f32>" } else { ty }.to_string(),
                )
            })
            .collect::<Vec<_>>()
    };
    let expected: Vec<_> = EXPECTED_FIELDS
        .iter()
        .map(|(name, ty)| (name.to_string(), ty.to_string()))
        .collect();
    assert_eq!(rust_fields(rust_source), expected);
    let swapped = rust_source
        .replace("pub map_world_width:", "pub temporary:")
        .replace("pub map_world_height:", "pub map_world_width:")
        .replace("pub temporary:", "pub map_world_height:");
    assert_ne!(rust_fields(&swapped), expected);
    assert_eq!(TerrainSurfaceUniform::METADATA.alignment().get(), 16);
    assert_eq!(TerrainSurfaceUniform::METADATA.min_size().get(), 160);
    for (i, offset) in [0, 4, 8, 12, 16, 20, 32, 48, 64, 80, 96, 112, 128, 144]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            TerrainSurfaceUniform::METADATA.offset(i),
            offset,
            "{}",
            EXPECTED_FIELDS[i].0
        );
    }
    let reordered = TYPES
        .replace("map_world_width:", "__temporary:")
        .replace("map_world_height:", "map_world_width:")
        .replace("__temporary:", "map_world_height:");
    assert_ne!(fields(&reordered), EXPECTED_FIELDS);
}
#[test]
fn terrain_binding_numbers_and_resources_are_stable_test() {
    let expected = [
        (100, "tsm", "TerrainSurfaceUniforms"),
        (101, "terrain_id_map", "texture_2d<f32>"),
        (102, "terrain_feature_map", "texture_2d<f32>"),
        (103, "grass_albedo", "texture_2d<f32>"),
        (104, "grass_sampler", "sampler"),
        (105, "dirt_albedo", "texture_2d<f32>"),
        (106, "dirt_sampler", "sampler"),
        (107, "sand_albedo", "texture_2d<f32>"),
        (108, "sand_sampler", "sampler"),
        (109, "river_albedo", "texture_2d<f32>"),
        (110, "river_sampler", "sampler"),
        (111, "terrain_macro_noise", "texture_2d<f32>"),
        (112, "macro_noise_sampler", "sampler"),
        (113, "grass_macro_overlay", "texture_2d<f32>"),
        (114, "grass_overlay_sampler", "sampler"),
        (115, "dirt_macro_overlay", "texture_2d<f32>"),
        (116, "dirt_overlay_sampler", "sampler"),
        (117, "sand_macro_overlay", "texture_2d<f32>"),
        (118, "sand_overlay_sampler", "sampler"),
        (119, "terrain_blend_mask_soft", "texture_2d<f32>"),
        (120, "blend_mask_sampler", "sampler"),
        (121, "river_flow_noise", "texture_2d<f32>"),
        (122, "river_flow_sampler", "sampler"),
        (123, "river_normal_like", "texture_2d<f32>"),
        (124, "river_normal_sampler", "sampler"),
        (125, "shoreline_detail", "texture_2d<f32>"),
        (126, "shoreline_detail_sampler", "sampler"),
        (127, "terrain_feature_lut", "texture_2d<f32>"),
        (128, "feature_lut_sampler", "sampler"),
        (129, "boundary_mask", "texture_2d<f32>"),
        (130, "boundary_mask_sampler", "sampler"),
        (131, "boundary_proximity_mask", "texture_2d<f32>"),
        (132, "boundary_proximity_sampler", "sampler"),
        (133, "indoor_light_field", "texture_2d<f32>"),
        (134, "indoor_light_sampler", "sampler"),
    ];
    assert_eq!(bindings(BINDINGS).unwrap(), expected);
    assert_ne!(
        bindings(&BINDINGS.replace("@binding(134)", "@binding(133)")).unwrap(),
        expected
    );
    assert!(bindings(&BINDINGS.replace("@group(#{MATERIAL_BIND_GROUP})", "@group(1)")).is_none());
    assert!(bindings(&BINDINGS.replace("var<uniform>", "var<storage>")).is_none());
    // Fix both slot kind and its owning Rust field, including sampler attributes.
    let expected_rust = [
        (100, "uniform", "uniforms"),
        (101, "texture", "terrain_id_map"),
        (102, "texture", "terrain_feature_map"),
        (103, "texture", "grass_albedo"),
        (104, "sampler", "grass_albedo"),
        (105, "texture", "dirt_albedo"),
        (106, "sampler", "dirt_albedo"),
        (107, "texture", "sand_albedo"),
        (108, "sampler", "sand_albedo"),
        (109, "texture", "river_albedo"),
        (110, "sampler", "river_albedo"),
        (111, "texture", "terrain_macro_noise"),
        (112, "sampler", "terrain_macro_noise"),
        (113, "texture", "grass_macro_overlay"),
        (114, "sampler", "grass_macro_overlay"),
        (115, "texture", "dirt_macro_overlay"),
        (116, "sampler", "dirt_macro_overlay"),
        (117, "texture", "sand_macro_overlay"),
        (118, "sampler", "sand_macro_overlay"),
        (119, "texture", "terrain_blend_mask_soft"),
        (120, "sampler", "terrain_blend_mask_soft"),
        (121, "texture", "river_flow_noise"),
        (122, "sampler", "river_flow_noise"),
        (123, "texture", "river_normal_like"),
        (124, "sampler", "river_normal_like"),
        (125, "texture", "shoreline_detail"),
        (126, "sampler", "shoreline_detail"),
        (127, "texture", "terrain_feature_lut"),
        (128, "sampler", "terrain_feature_lut"),
        (129, "texture", "boundary_mask"),
        (130, "sampler", "boundary_mask"),
        (131, "texture", "boundary_proximity_mask"),
        (132, "sampler", "boundary_proximity_mask"),
        (133, "texture", "indoor_light_field"),
        (134, "sampler", "indoor_light_field"),
    ];
    let rust_source = include_str!("terrain_surface_material.rs");
    for material in [
        "TerrainSurfaceMaterialExt",
        "TerrainSurfaceMaterialExtLod1Lite",
        "TerrainSurfaceMaterialExtLod2",
    ] {
        let body = rust_struct(rust_source, material);
        assert_eq!(rust_bindings(body).unwrap(), expected_rust, "{material}");
        let swapped = body.replace("#[texture(103)]", "#[sampler(103)]");
        assert_ne!(rust_bindings(&swapped).unwrap(), expected_rust);
    }
    assert!(
        BINDINGS.contains("#import \"shaders/terrain_surface_types.wgsl\"::TerrainSurfaceUniforms")
    );
    let prepass = include_str!("../../../../assets/shaders/terrain_surface_material_prepass.wgsl");
    assert!(
        prepass.contains("#import \"shaders/terrain_surface_types.wgsl\"::TerrainSurfaceUniforms")
    );
    assert_eq!(
        bindings(prepass).unwrap(),
        [(100, "terrain_surface", "TerrainSurfaceUniforms")]
    );
    assert!(!prepass.contains("terrain_surface_bindings"));
}
