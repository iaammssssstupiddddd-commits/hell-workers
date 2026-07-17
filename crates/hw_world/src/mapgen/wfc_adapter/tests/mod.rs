use super::*;
use crate::mapgen::pipeline::generate_world_layout;
use crate::test_seeds::GOLDEN_SEED_PRIMARY;

fn mix_checksum(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
}

#[test]
fn golden_layout_terrain_and_masks_remain_stable() {
    let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;

    for terrain in &layout.terrain_tiles {
        let value = match terrain {
            TerrainType::Grass => 0,
            TerrainType::Dirt => 1,
            TerrainType::Sand => 2,
            TerrainType::River => 3,
        };
        mix_checksum(&mut checksum, value);
    }
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let pos = (x, y);
            let mask_bits = u8::from(layout.masks.river_mask.get(pos))
                | (u8::from(layout.masks.final_sand_mask.get(pos)) << 1)
                | (u8::from(layout.masks.inland_sand_mask.get(pos)) << 2)
                | (u8::from(layout.masks.grass_zone_mask.get(pos)) << 3)
                | (u8::from(layout.masks.dirt_zone_mask.get(pos)) << 4)
                | (u8::from(layout.masks.rock_field_mask.get(pos)) << 5);
            mix_checksum(&mut checksum, mask_bits);
        }
    }

    assert_eq!(
        checksum, 5_792_967_092_046_543_807,
        "update only when the map contract intentionally changes"
    );
}

#[test]
fn generated_layouts_have_no_visual_cross() {
    for seed in [0u64, 1, 42, 999, GOLDEN_SEED_PRIMARY] {
        let layout = generate_world_layout(seed);
        assert!(
            !has_any_visual_cross_2x2(&layout.terrain_tiles, &layout.masks),
            "seed={seed} has visual cross boundary"
        );
    }
}
