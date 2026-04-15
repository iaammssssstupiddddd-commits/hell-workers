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

fn soul_projected_shadow_amount(
    in: pbr_types::PbrInput,
    soul_shadow_projectors: array<vec4<f32>, 12>,
    soul_shadow_projector_meta: vec4<f32>,
) -> f32 {
    let projector_count = u32(clamp(soul_shadow_projector_meta.x, 0.0, 12.0));
    if projector_count == 0u {
        return 0.0;
    }

    let feather = max(soul_shadow_projector_meta.y, 0.001);
    let strength = clamp(soul_shadow_projector_meta.z, 0.0, 1.0);
    if strength <= 0.0 {
        return 0.0;
    }

    let forward_extent = max(soul_shadow_projector_meta.w, 0.001);
    let n_directional_lights = view_bindings::lights.n_directional_lights;
    var found_shadow_light = false;
    var projector_weight = 0.0;

    for (var light_index: u32 = 0u; light_index < n_directional_lights; light_index = light_index + 1u) {
        let light = &view_bindings::lights.directional_lights[light_index];
        if ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) == 0u {
            continue;
        }

        found_shadow_light = true;
        let shadow_dir = -(*light).direction_to_light.xyz;
        for (var projector_index: u32 = 0u; projector_index < projector_count; projector_index = projector_index + 1u) {
            let projector = soul_shadow_projectors[projector_index];
            let to_fragment = in.world_position.xyz - projector.xyz;
            let along_shadow = dot(to_fragment, shadow_dir);
            if along_shadow < -feather || along_shadow > forward_extent + feather * 3.0 {
                continue;
            }

            let lateral = to_fragment - shadow_dir * along_shadow;
            let radius = max(projector.w, 0.001);
            let forward_t = clamp(along_shadow / forward_extent, 0.0, 1.0);
            let lateral_radius = mix(radius * 0.62, radius * 1.18, forward_t);
            let inner_radius = max(lateral_radius - feather, 0.0);
            let radial_weight = 1.0 - smoothstep(inner_radius, lateral_radius, length(lateral));
            let start_weight = smoothstep(-feather, feather * 1.5, along_shadow);
            let end_weight =
                1.0 - smoothstep(forward_extent - feather * 1.5, forward_extent + feather * 2.0, along_shadow);
            projector_weight = max(projector_weight, radial_weight * start_weight * end_weight);
        }
    }

    if !found_shadow_light {
        return 0.0;
    }

    return projector_weight * strength;
}

fn apply_soul_projected_shadow(
    in: pbr_types::PbrInput,
    lit_rgb: vec3<f32>,
    shadow_style_tint: vec4<f32>,
    soul_shadow_projectors: array<vec4<f32>, 12>,
    soul_shadow_projector_meta: vec4<f32>,
) -> vec3<f32> {
    let projected_shadow = soul_projected_shadow_amount(
        in,
        soul_shadow_projectors,
        soul_shadow_projector_meta,
    );
    if projected_shadow <= 0.0 {
        return lit_rgb;
    }

    let projector_opacity = smoothstep(0.02, 0.24, projected_shadow);
    return mix(lit_rgb, vec3<f32>(0.0), projector_opacity * 0.96);
}

fn apply_directional_shadow_style(
    in: pbr_types::PbrInput,
    lit_rgb: vec3<f32>,
    shadow_style_params: vec4<f32>,
    shadow_style_tint: vec4<f32>,
    shadow_style_blur: vec4<f32>,
    soul_shadow_projectors: array<vec4<f32>, 12>,
    soul_shadow_projector_meta: vec4<f32>,
) -> vec3<f32> {
    let style_mix = clamp(shadow_style_params.x, 0.0, 1.0);
    if style_mix <= 0.0 {
        return apply_soul_projected_shadow(
            in,
            lit_rgb,
            shadow_style_tint,
            soul_shadow_projectors,
            soul_shadow_projector_meta,
        );
    }

    let blur_radius_texels = max(shadow_style_blur.x, 0.0);
    let outer_shadow_amount = 1.0 - directional_shadow_visibility(in, blur_radius_texels);
    if outer_shadow_amount <= 0.0 {
        return apply_soul_projected_shadow(
            in,
            lit_rgb,
            shadow_style_tint,
            soul_shadow_projectors,
            soul_shadow_projector_meta,
        );
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
        return apply_soul_projected_shadow(
            in,
            lit_rgb,
            shadow_style_tint,
            soul_shadow_projectors,
            soul_shadow_projector_meta,
        );
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
    return apply_soul_projected_shadow(
        in,
        styled,
        shadow_style_tint,
        soul_shadow_projectors,
        soul_shadow_projector_meta,
    );
}
