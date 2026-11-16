//! One-time seeded stockpiles and passive observation for target-binding input.
use super::*;
use crate::systems::logistics::{BelongsTo, Stockpile};
use hw_core::constants::{TILE_SIZE, Z_MAP};
use hw_logistics::{StockpileAcceptance, StockpilePolicy};
use hw_ui::intents::{StockpilePolicyEditTarget, UiIntent};
use hw_world::{WorldMap, zones::Yard};

#[derive(Component)]
struct FixtureCell {
    index: usize,
    grid: (i32, i32),
}

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("refactor-stockpile-target")
}

pub(super) fn prepare(world: &mut World) {
    let cells = {
        let map = world.resource::<WorldMap>();
        (12..60)
            .flat_map(|y| (12..55).map(move |x| [(x, y), (x + 5, y)]))
            .find(|cells| {
                cells.iter().all(|&p| {
                    map.is_walkable(p.0, p.1) && !map.has_building(p) && !map.has_stockpile(p)
                })
            })
            .expect("stockpile fixture requires two empty cells")
    };
    let a = WorldMap::grid_to_world(cells[0].0, cells[0].1);
    let b = WorldMap::grid_to_world(cells[1].0, cells[1].1);
    let yard = world
        .spawn(Yard {
            min: a - Vec2::splat(TILE_SIZE),
            max: b + Vec2::splat(TILE_SIZE),
        })
        .id();
    for (index, grid) in cells.into_iter().enumerate() {
        let pos = WorldMap::grid_to_world(grid.0, grid.1);
        let entity = world
            .spawn((
                FixtureCell { index, grid },
                Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                StockpilePolicy::for_capacity(10),
                BelongsTo(yard),
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                Name::new("Stockpile"),
            ))
            .id();
        world
            .resource_mut::<WorldMap>()
            .register_stockpile_tile(grid, entity);
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        camera.translation.x = (a.x + b.x) / 2.;
        camera.translation.y = a.y;
    }
    world.resource_mut::<Time<Virtual>>().pause();
}

pub(super) fn snapshot(world: &mut World, viewport: Vec2) -> Value {
    if !enabled() {
        return Value::Null;
    }
    let cells: Vec<_> = world
        .query::<(Entity, &FixtureCell, &StockpilePolicy)>()
        .iter(world)
        .map(|(entity, cell, policy)| (entity, cell.index, cell.grid, *policy))
        .collect();
    let mut result = Vec::new();
    for (entity, index, grid, policy) in cells {
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        let target =
            crate::interface::selection::native_direct_owner_point(world, entity, &[position]);
        result.push(json!({"entity": entity.to_bits(), "index": index, "grid": [grid.0, grid.1],
            "map_owner": world.resource::<WorldMap>().stockpile_entity(grid).map(Entity::to_bits),
            "acceptance": format!("{:?}", policy.acceptance), "priority": format!("{:?}", policy.inbound_priority),
            "is_none": policy.acceptance == StockpileAcceptance::none(),
            "is_any": policy.acceptance == StockpileAcceptance::Any,
            "target_amount": policy.target_amount, "allow_export": policy.allow_export,
            "point": target.map(|(_, screen, rank)| json!({"screen": screen.to_array(), "rank": rank}))}));
    }
    result.sort_by_key(|v| v["index"].as_u64().unwrap_or(u64::MAX));
    let button = world
        .resource::<InfoPanelNodes>()
        .stockpile
        .stockpile_acceptance_none_button;
    let button = button.and_then(|node| {
        let action = &world.get::<MenuButton>(node)?.0;
        let UiIntent::ApplyStockpilePolicy { target: StockpilePolicyEditTarget::Single(target), patch } = action else { return None; };
        Some(json!({"entity": node.to_bits(), "key": format!("menu:{action:?}"),
            "rect": visible_rect(world, node, viewport), "target": target.to_bits(),
            "clear": patch.acceptance == Some(StockpileAcceptance::none()),
            "other_empty": patch.inbound_priority.is_none() && patch.target_amount.is_none() && patch.allow_export.is_none()}))
    });
    json!({"cells": result, "button": button})
}
