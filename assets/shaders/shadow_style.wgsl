#import bevy_pbr::{
    mesh_types,
    mesh_view_bindings as view_bindings,
    mesh_view_types,
    pbr_types,
    shadow_sampling,
    shadows,
}

fn shadow_style_luma(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3(0.299, 0.587, 0.114));
}

fn shadow_style_view_z(in: pbr_types::PbrInput) -> f32 {
    return dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z,
    ), in.world_position);
}

fn sample_directional_shadow_blurred(
    light_id: u32,
    in: pbr_types::PbrInput,
    view_z: f32,
    blur_radius_texels: f32,
) -> f32 {
    let light = &view_bindings::lights.directional_lights[light_id];
    let cascade_index = shadows::get_cascade_index(light_id, view_z);

    if cascade_index >= (*light).num_cascades {
        return 1.0;
    }

    let cascade = &(*light).cascades[cascade_index];
    let normal_offset = (*light).shadow_normal_bias * (*cascade).texel_size * in.world_normal.xyz;
    let depth_offset = (*light).shadow_depth_bias * (*light).direction_to_light.xyz;
    let offset_position = vec4<f32>(
        in.world_position.xyz + normal_offset + depth_offset,
        in.world_position.w,
    );

    let light_local = shadows::world_to_directional_light_local(
        light_id,
        cascade_index,
        offset_position,
    );
    if light_local.w == 0.0 {
        return 1.0;
    }

    let array_index = i32((*light).depth_texture_base_index + cascade_index);
    if blur_radius_texels <= 0.5 {
        return shadow_sampling::sample_shadow_map_hardware(
            light_local.xy,
            light_local.z,
            array_index,
        );
    }

    let blur_uv = (*cascade).texel_size * blur_radius_texels;
    let offset_x = vec2<f32>(blur_uv, 0.0);
    let offset_y = vec2<f32>(0.0, blur_uv);
    let offset_d0 = vec2<f32>(blur_uv, blur_uv);
    let offset_d1 = vec2<f32>(blur_uv, -blur_uv);

    var visibility = 0.0;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy,
        light_local.z,
        array_index,
    ) * 0.20;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy + offset_x,
        light_local.z,
        array_index,
    ) * 0.12;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy - offset_x,
        light_local.z,
        array_index,
    ) * 0.12;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy + offset_y,
        light_local.z,
        array_index,
    ) * 0.12;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy - offset_y,
        light_local.z,
        array_index,
    ) * 0.12;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy + offset_d0,
        light_local.z,
        array_index,
    ) * 0.08;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy - offset_d0,
        light_local.z,
        array_index,
    ) * 0.08;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy + offset_d1,
        light_local.z,
        array_index,
    ) * 0.08;
    visibility += shadow_sampling::sample_shadow_map_hardware(
        light_local.xy - offset_d1,
        light_local.z,
        array_index,
    ) * 0.08;
    return visibility;
}

fn directional_shadow_visibility(
    in: pbr_types::PbrInput,
    blur_radius_texels: f32,
) -> f32 {
    if (in.flags & mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT) == 0u {
        return 1.0;
    }

    let view_z = shadow_style_view_z(in);

    let n_directional_lights = view_bindings::lights.n_directional_lights;
    var found_shadow_light = false;
    var visibility = 1.0;

    for (var i: u32 = 0u; i < n_directional_lights; i = i + 1u) {
        let light = &view_bindings::lights.directional_lights[i];
        if ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) == 0u {
            continue;
        }

        found_shadow_light = true;
        visibility = min(
            visibility,
            sample_directional_shadow_blurred(i, in, view_z, blur_radius_texels),
        );
    }

    if !found_shadow_light {
        return 1.0;
    }

    return visibility;
}

fn apply_directional_shadow_style(
    in: pbr_types::PbrInput,
    lit_rgb: vec3<f32>,
    shadow_style_params: vec4<f32>,
    shadow_style_tint: vec4<f32>,
    shadow_style_blur: vec4<f32>,
) -> vec3<f32> {
    let style_mix = clamp(shadow_style_params.x, 0.0, 1.0);
    if style_mix <= 0.0 {
        return lit_rgb;
    }

    let blur_radius_texels = max(shadow_style_blur.x, 0.0);
    let outer_shadow_amount = 1.0 - directional_shadow_visibility(in, blur_radius_texels);
    if outer_shadow_amount <= 0.0 {
        return lit_rgb;
    }

    let inner_blur_radius_texels = blur_radius_texels * 0.35;
    let inner_shadow_amount = 1.0 - directional_shadow_visibility(in, inner_blur_radius_texels);

    let threshold = clamp(shadow_style_params.y, 0.0, 1.0);
    let softness = max(shadow_style_params.z, 0.0001);
    let darken = clamp(shadow_style_params.w, 0.0, 1.0);
    let shadow_mask = smoothstep(
        threshold - softness,
        threshold + softness,
        outer_shadow_amount,
    );

    if shadow_mask <= 0.0 {
        return lit_rgb;
    }

    let core_ratio = clamp(
        inner_shadow_amount / max(outer_shadow_amount, 0.0001),
        0.0,
        1.0,
    );
    let shadow_core = smoothstep(0.38, 0.92, core_ratio);
    let shadow_opacity = shadow_mask * pow(shadow_core, 2.6);
    let darkened = lit_rgb * mix(1.0, darken, shadow_opacity);
    let tint_rgb = clamp(shadow_style_tint.rgb, vec3(0.0), vec3(1.5));
    let tint_strength = clamp(shadow_style_tint.a, 0.0, 1.0);
    let tint_target = shadow_style_luma(darkened) * tint_rgb;
    let tinted = mix(darkened, tint_target, tint_strength * shadow_opacity);
    let styled = mix(lit_rgb, tinted, style_mix * shadow_opacity);
    return styled;
}
