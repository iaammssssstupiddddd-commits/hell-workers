use super::*;
use crate::anchor::AnchorLayout;
use crate::mapgen::generate_world_layout;
use crate::mapgen::types::ResourceSpawnCandidates;
use crate::mapgen::validate::validate_post_resource;
use crate::mapgen::wfc_adapter::fallback_terrain;
use crate::test_seeds::{GOLDEN_SEED_PRIMARY, GOLDEN_SEED_SECONDARY};
use crate::world_masks::WorldMasks;

fn make_fallback_layout(seed: u64) -> GeneratedWorldLayout {
    let anchors = AnchorLayout::aligned_to_worldgen_seed(seed);
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(seed);
    masks.fill_sand_from_river_seed(seed);
    masks.fill_terrain_zones_from_seed(seed);
    masks.fill_rock_fields_from_seed(seed);

    let candidate = GeneratedWorldLayout {
        terrain_tiles: fallback_terrain(&masks, seed),
        anchors,
        masks,
        resource_spawn_candidates: ResourceSpawnCandidates::default(),
        initial_tree_positions: Vec::new(),
        forest_regrowth_zones: Vec::new(),
        initial_rock_positions: Vec::new(),
        master_seed: seed,
        generation_attempt: 65,
        used_fallback: true,
    };

    let validated =
        crate::mapgen::validate::lightweight_validate(&candidate).expect("fallback terrain");

    GeneratedWorldLayout {
        resource_spawn_candidates: validated,
        ..candidate
    }
}

#[test]
fn trees_not_in_exclusion_zone() {
    let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
    assert!(
        !layout.used_fallback,
        "seed={GOLDEN_SEED_PRIMARY}: fallback が使われた"
    );
    for &pos in &layout.initial_tree_positions {
        assert!(
            !layout.masks.anchor_mask.get(pos),
            "tree at {pos:?} is inside anchor_mask"
        );
        assert!(
            !layout.masks.tree_dense_protection_band.get(pos),
            "tree at {pos:?} is inside tree_dense_protection_band"
        );
        assert!(
            !layout.masks.river_mask.get(pos),
            "tree at {pos:?} is inside river_mask"
        );
        assert!(
            !layout.masks.final_sand_mask.get(pos),
            "tree at {pos:?} is inside final_sand_mask"
        );
        assert!(
            !layout.masks.inland_sand_mask.get(pos),
            "tree at {pos:?} is inside inland_sand_mask"
        );
    }
}

#[test]
fn trees_are_inside_some_forest_zone() {
    let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
    assert!(!layout.used_fallback);
    for &pos in &layout.initial_tree_positions {
        assert!(
            layout.forest_regrowth_zones.iter().any(|z| z.contains(pos)),
            "tree at {pos:?} is outside all forest_regrowth_zones"
        );
    }
}

#[test]
fn rocks_not_in_exclusion_zone() {
    let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
    assert!(!layout.used_fallback);
    for &pos in &layout.initial_rock_positions {
        assert!(
            !layout.masks.anchor_mask.get(pos),
            "rock at {pos:?} is inside anchor_mask"
        );
        assert!(
            !layout.masks.rock_protection_band.get(pos),
            "rock at {pos:?} is inside rock_protection_band"
        );
        assert!(
            !layout.masks.river_mask.get(pos),
            "rock at {pos:?} is inside river_mask"
        );
        assert!(
            !layout.masks.final_sand_mask.get(pos),
            "rock at {pos:?} is inside final_sand_mask"
        );
        assert!(
            !layout.masks.inland_sand_mask.get(pos),
            "rock at {pos:?} is inside inland_sand_mask"
        );
    }
}

#[test]
fn rocks_match_rock_field_mask() {
    let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
    assert!(!layout.used_fallback);
    for &pos in &layout.initial_rock_positions {
        assert!(
            layout.masks.rock_field_mask.get(pos),
            "rock at {pos:?} is outside rock_field_mask"
        );
    }
    assert_eq!(
        layout.initial_rock_positions.len(),
        layout.masks.rock_field_mask.count_set(),
        "all rock_field_mask cells should materialize as rocks"
    );
}

#[test]
fn rock_field_mask_is_dirt_in_final_layout() {
    let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.rock_field_mask.get((x, y)) {
                assert_eq!(
                    layout.terrain_tiles[(y * MAP_WIDTH + x) as usize],
                    TerrainType::Dirt,
                    "rock_field_mask cell ({x},{y}) is not Dirt"
                );
            }
        }
    }
}

#[test]
fn resource_layout_keeps_required_paths_open() {
    for seed in [GOLDEN_SEED_PRIMARY, GOLDEN_SEED_SECONDARY] {
        let layout = generate_world_layout(seed);
        assert!(!layout.used_fallback, "seed={seed}: fallback が使われた");
        assert!(
            !layout.initial_tree_positions.is_empty(),
            "seed={seed}: initial_tree_positions が空"
        );
        assert!(
            !layout.forest_regrowth_zones.is_empty(),
            "seed={seed}: forest_regrowth_zones が空"
        );
        assert!(
            !layout.initial_rock_positions.is_empty(),
            "seed={seed}: initial_rock_positions が空"
        );
        let res = ResourceLayout {
            initial_tree_positions: layout.initial_tree_positions.clone(),
            forest_regrowth_zones: layout.forest_regrowth_zones.clone(),
            initial_rock_positions: layout.initial_rock_positions.clone(),
            rock_candidates: layout.resource_spawn_candidates.rock_candidates.clone(),
        };
        assert!(
            validate_post_resource(&layout, &res).is_ok(),
            "seed={seed}: validate_post_resource failed"
        );
    }
}

#[test]
fn rock_candidates_equals_initial_rock_positions() {
    let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
    assert!(!layout.used_fallback);
    let mut expected = layout.initial_rock_positions.clone();
    let mut actual = layout.resource_spawn_candidates.rock_candidates.clone();
    expected.sort();
    actual.sort();
    assert_eq!(expected, actual);
}

#[test]
fn resource_layout_is_deterministic() {
    let l1 = generate_world_layout(GOLDEN_SEED_PRIMARY);
    let l2 = generate_world_layout(GOLDEN_SEED_PRIMARY);
    assert_eq!(l1.initial_tree_positions, l2.initial_tree_positions);
    assert_eq!(l1.initial_rock_positions, l2.initial_rock_positions);
    assert_eq!(
        l1.forest_regrowth_zones
            .iter()
            .map(|z| (z.center, z.radius))
            .collect::<Vec<_>>(),
        l2.forest_regrowth_zones
            .iter()
            .map(|z| (z.center, z.radius))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn fallback_resource_layout_is_non_empty_and_paths_open() {
    for seed in [0u64, GOLDEN_SEED_PRIMARY, 99, GOLDEN_SEED_SECONDARY] {
        let layout = make_fallback_layout(seed);
        let res = generate_resource_layout_fallback(&layout, seed)
            .expect("fallback resource generation must succeed for representative seeds");

        assert!(
            !res.initial_tree_positions.is_empty(),
            "seed={seed}: fallback trees are empty"
        );
        assert!(
            !res.forest_regrowth_zones.is_empty(),
            "seed={seed}: fallback forest zones are empty"
        );
        assert!(
            !res.initial_rock_positions.is_empty(),
            "seed={seed}: fallback rocks are empty"
        );
        assert!(
            validate_post_resource(&layout, &res).is_ok(),
            "seed={seed}: fallback resource layout broke required paths"
        );
    }
}
