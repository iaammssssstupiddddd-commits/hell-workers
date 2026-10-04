struct TerrainSurfaceUniforms {
    map_world_width:            f32,
    map_world_height:           f32,
    uv_scale:                   f32,
    blend_strength:             f32,
    macro_noise_scale:          f32,
    overlay_scale:              f32,
    // LUT uniform 定数（sync_terrain_feature_lut_uniforms_system が設定）
    lut_shore:                  vec4<f32>,
    lut_inland:                 vec4<f32>,
    lut_rock:                   vec4<f32>,
    feature_lut_constants_ready: f32,
    shadow_style_params:        vec4<f32>,
    shadow_style_tint:          vec4<f32>,
    shadow_style_blur:          vec4<f32>,
    indoor_light_params:        vec4<f32>,
}
