use super::*;

/// The subset of loaded assets needed to recreate a regular building Blueprint.
///
/// Construction state itself is asset-independent; keeping this narrow lets the
/// rehydration path be tested without constructing the full `GameAssets` catalog.
#[derive(Default)]
pub(super) struct BlueprintSpriteHandles {
    wall_isolated: Handle<Image>,
    door_closed: Handle<Image>,
    mud_floor: Handle<Image>,
    tank_empty: Handle<Image>,
    mud_mixer: Handle<Image>,
    rest_area: Handle<Image>,
    bridge: Handle<Image>,
    sand_pile: Handle<Image>,
    bone_pile: Handle<Image>,
    wheelbarrow_parking: Handle<Image>,
}

impl From<&GameAssets> for BlueprintSpriteHandles {
    fn from(assets: &GameAssets) -> Self {
        Self {
            wall_isolated: assets.wall_isolated.clone(),
            door_closed: assets.door_closed.clone(),
            mud_floor: assets.mud_floor.clone(),
            tank_empty: assets.tank_empty.clone(),
            mud_mixer: assets.mud_mixer.clone(),
            rest_area: assets.rest_area.clone(),
            bridge: assets.bridge.clone(),
            sand_pile: assets.sand_pile.clone(),
            bone_pile: assets.bone_pile.clone(),
            wheelbarrow_parking: assets.wheelbarrow_parking.clone(),
        }
    }
}

impl BlueprintSpriteHandles {
    fn sprite(&self, kind: BuildingType) -> Sprite {
        let image = match kind {
            BuildingType::Wall => self.wall_isolated.clone(),
            BuildingType::Door => self.door_closed.clone(),
            BuildingType::Floor => self.mud_floor.clone(),
            BuildingType::Tank => self.tank_empty.clone(),
            BuildingType::MudMixer => self.mud_mixer.clone(),
            BuildingType::RestArea => self.rest_area.clone(),
            BuildingType::Bridge => self.bridge.clone(),
            BuildingType::SandPile => self.sand_pile.clone(),
            BuildingType::BonePile | BuildingType::SoulSpa | BuildingType::OutdoorLamp => {
                self.bone_pile.clone()
            }
            BuildingType::WheelbarrowParking => self.wheelbarrow_parking.clone(),
        };

        Sprite {
            image,
            color: Color::srgba(1.0, 1.0, 1.0, 0.5),
            custom_size: Some(building_size(kind)),
            ..default()
        }
    }
}

/// Restores the non-persistent visual shell for construction roots.
///
/// Mirrors are built directly from their durable source rather than waiting for
/// `GameSystemSet::Logic`: a load can occur while virtual time is paused, but
/// the visual systems must still render the saved construction state.
pub(super) fn rehydrate_construction_shells(
    world: &mut World,
    sprite_handles: &BlueprintSpriteHandles,
) {
    let blueprints: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &Blueprint,
            Option<&BlueprintVisualState>,
            Option<&Sprite>,
            Option<&BlueprintVisual>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(
                |(entity, blueprint, existing_visual_state, sprite, visual, name)| {
                    let visual_state = existing_visual_state
                        .is_none()
                        .then(|| blueprint_visual_state(blueprint));
                    let sprite = sprite
                        .is_none()
                        .then(|| sprite_handles.sprite(blueprint.kind));
                    let visual = visual.is_none().then(|| {
                        let state = visual_state
                            .as_ref()
                            .or(existing_visual_state)
                            .expect("a BlueprintVisual requires a visual state");
                        BlueprintVisual::from_visual_state(state)
                    });
                    let name = name
                        .is_none()
                        .then(|| Name::new(format!("Blueprint ({:?})", blueprint.kind)));
                    (visual_state.is_some()
                        || sprite.is_some()
                        || visual.is_some()
                        || name.is_some())
                    .then_some((entity, visual_state, sprite, visual, name))
                },
            )
            .collect()
    };

    let floor_sites: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &FloorConstructionSite,
            Option<&FloorSiteVisualState>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, site, visual_state, name)| {
                let visual_state = visual_state
                    .is_none()
                    .then(|| floor_site_visual_state(site));
                let name = name.is_none().then(|| Name::new("FloorConstructionSite"));
                (visual_state.is_some() || name.is_some()).then_some((entity, visual_state, name))
            })
            .collect()
    };

    let floor_tiles: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &FloorTileBlueprint,
            Option<&FloorTileVisualMirror>,
            Option<&Sprite>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, tile, visual_state, sprite, name)| {
                let visual_state = visual_state
                    .is_none()
                    .then(|| floor_tile_visual_mirror(tile));
                let sprite = sprite.is_none().then(|| Sprite {
                    color: Color::srgba(0.50, 0.50, 0.80, 0.20),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                });
                let name = name.is_none().then(|| {
                    Name::new(format!(
                        "FloorTile({},{})",
                        tile.grid_pos.0, tile.grid_pos.1
                    ))
                });
                (visual_state.is_some() || sprite.is_some() || name.is_some()).then_some((
                    entity,
                    visual_state,
                    sprite,
                    name,
                ))
            })
            .collect()
    };

    let wall_sites: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &WallConstructionSite,
            Option<&WallSiteVisualState>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, site, visual_state, name)| {
                let visual_state = visual_state.is_none().then(|| wall_site_visual_state(site));
                let name = name.is_none().then(|| Name::new("WallConstructionSite"));
                (visual_state.is_some() || name.is_some()).then_some((entity, visual_state, name))
            })
            .collect()
    };

    let wall_tiles: Vec<_> = {
        let mut query = world.query::<(
            Entity,
            &WallTileBlueprint,
            Option<&WallTileVisualMirror>,
            Option<&Sprite>,
            Option<&Name>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, tile, visual_state, sprite, name)| {
                let visual_state = visual_state
                    .is_none()
                    .then(|| wall_tile_visual_mirror(tile));
                let sprite = sprite.is_none().then(|| Sprite {
                    color: Color::srgba(0.80, 0.55, 0.30, 0.25),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                });
                let name = name.is_none().then(|| {
                    Name::new(format!("WallTile({},{})", tile.grid_pos.0, tile.grid_pos.1))
                });
                (visual_state.is_some() || sprite.is_some() || name.is_some()).then_some((
                    entity,
                    visual_state,
                    sprite,
                    name,
                ))
            })
            .collect()
    };

    let mut commands = world.commands();
    for (entity, visual_state, sprite, visual, name) in blueprints {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(sprite) = sprite {
            commands.entity(entity).insert(sprite);
        }
        if let Some(visual) = visual {
            commands.entity(entity).insert(visual);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, name) in floor_sites {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, sprite, name) in floor_tiles {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(sprite) = sprite {
            commands.entity(entity).insert(sprite);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, name) in wall_sites {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
    for (entity, visual_state, sprite, name) in wall_tiles {
        if let Some(visual_state) = visual_state {
            commands.entity(entity).insert(visual_state);
        }
        if let Some(sprite) = sprite {
            commands.entity(entity).insert(sprite);
        }
        if let Some(name) = name {
            commands.entity(entity).insert(name);
        }
    }
}
