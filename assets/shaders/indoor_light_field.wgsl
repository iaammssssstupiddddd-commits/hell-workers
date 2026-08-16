#define_import_path hell_workers::indoor_light_field

const INDOOR_LIGHT_ANCHOR_BIT: u32 = 0x80000000u;
const INDOOR_LIGHT_POLICY_SHIFT: u32 = 18u;
const INDOOR_LIGHT_POLICY_MASK: u32 = 0x3u;
const INDOOR_LIGHT_POLICY_WALL: u32 = 1u;
const INDOOR_LIGHT_POLICY_DOOR: u32 = 2u;

fn indoor_light_world_cell(
    world_xz: vec2<f32>,
    dimensions: vec2<u32>,
    tile_size: f32,
) -> vec2<i32> {
    let dimensions_f = vec2<f32>(dimensions);
    let map_origin = vec2<f32>(
        -dimensions_f.x * tile_size * 0.5,
        dimensions_f.y * tile_size * 0.5,
    );
    return vec2<i32>(floor(vec2<f32>(
        (world_xz.x - map_origin.x) / tile_size,
        (map_origin.y - world_xz.y) / tile_size,
    )));
}

fn indoor_light_sample_cell(
    field: texture_2d<f32>,
    field_sampler: sampler,
    cell: vec2<i32>,
) -> vec3<f32> {
    let dimensions = textureDimensions(field);
    if (
        cell.x < 0 || cell.y < 0
        || cell.x >= i32(dimensions.x) || cell.y >= i32(dimensions.y)
    ) {
        return vec3<f32>(0.0);
    }
    let uv = (vec2<f32>(cell) + vec2<f32>(0.5)) / vec2<f32>(dimensions);
    let sample = textureSampleLevel(field, field_sampler, uv, 0.0);
    return sample.rgb * sample.a;
}

fn indoor_light_max_luminance_neighbor(
    field: texture_2d<f32>,
    field_sampler: sampler,
    root: vec2<i32>,
) -> vec3<f32> {
    // Stable tie order: North, East, South, West.
    let offsets = array<vec2<i32>, 4>(
        vec2<i32>(0, 1),
        vec2<i32>(1, 0),
        vec2<i32>(0, -1),
        vec2<i32>(-1, 0),
    );
    var selected = vec3<f32>(0.0);
    var selected_luminance = -1.0;
    for (var index = 0u; index < 4u; index = index + 1u) {
        let candidate = indoor_light_sample_cell(field, field_sampler, root + offsets[index]);
        let luminance = dot(candidate, vec3<f32>(0.2126, 0.7152, 0.0722));
        if luminance > selected_luminance {
            selected = candidate;
            selected_luminance = luminance;
        }
    }
    return selected;
}

fn indoor_light_cardinal_delta(direction: u32) -> vec2<i32> {
    switch direction & 0x3u {
        case 0u: { return vec2<i32>(0, 1); }
        case 1u: { return vec2<i32>(1, 0); }
        case 2u: { return vec2<i32>(0, -1); }
        default: { return vec2<i32>(-1, 0); }
    }
}

fn indoor_light_surface_delta(world_normal: vec3<f32>) -> vec2<i32> {
    if abs(world_normal.x) >= abs(world_normal.z) {
        return vec2<i32>(select(-1, 1, world_normal.x >= 0.0), 0);
    }
    // Simulation +Y is presentation -Z.
    return vec2<i32>(0, select(-1, 1, world_normal.z <= 0.0));
}

fn sample_indoor_light_field(
    field: texture_2d<f32>,
    field_sampler: sampler,
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
    mesh_tag: u32,
    tile_size: f32,
) -> vec3<f32> {
    let dimensions = textureDimensions(field);
    var root = indoor_light_world_cell(world_position.xz, dimensions, tile_size);
    let anchored = (mesh_tag & INDOOR_LIGHT_ANCHOR_BIT) != 0u;
    let policy = (mesh_tag >> INDOOR_LIGHT_POLICY_SHIFT) & INDOOR_LIGHT_POLICY_MASK;
    if anchored {
        root = vec2<i32>(i32(mesh_tag & 0xffu), i32((mesh_tag >> 8u) & 0xffu));
    }

    if anchored && (policy == INDOOR_LIGHT_POLICY_WALL || policy == INDOOR_LIGHT_POLICY_DOOR) {
        if abs(world_normal.y) > 0.5 {
            return indoor_light_max_luminance_neighbor(field, field_sampler, root);
        }
        var delta = indoor_light_surface_delta(world_normal);
        if policy == INDOOR_LIGHT_POLICY_DOOR {
            delta = indoor_light_cardinal_delta((mesh_tag >> 16u) & 0x3u);
        }
        return indoor_light_sample_cell(field, field_sampler, root + delta);
    }

    return indoor_light_sample_cell(field, field_sampler, root);
}
