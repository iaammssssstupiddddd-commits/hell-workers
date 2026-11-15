//! Seeded, no-input art readability only. Production owns the model and Text.
use super::*;
use hw_energy::{ConsumesFrom, PowerConsumer, PowerShedReason, PowerSupplyState};
use hw_ui::{
    components::InfoPanelNodes,
    models::inspection::EntityInspectionViewModel,
    power::{PowerInspectionRole, PowerShedReasonValue, PowerSupplyStateValue},
    shell::{UiShellState, WorkspacePage},
};

fn supply(state: PowerSupplyState) -> (PowerSupplyStateValue, &'static str) {
    match state {
        PowerSupplyState::Supplied => (PowerSupplyStateValue::Supplied, "Supply: Supplied"),
        PowerSupplyState::Shed { reason } => {
            let (reason, label) = match reason {
                PowerShedReason::InsufficientGeneration => (
                    PowerShedReasonValue::InsufficientGeneration,
                    "Supply: Shed — insufficient generation",
                ),
                PowerShedReason::RestoreMargin => (
                    PowerShedReasonValue::RestoreMargin,
                    "Supply: Shed — waiting for restore margin",
                ),
                PowerShedReason::LegacyGlobalDeficit => (
                    PowerShedReasonValue::LegacyGlobalDeficit,
                    "Supply: Shed — legacy grid deficit",
                ),
            };
            (PowerSupplyStateValue::Shed { reason }, label)
        }
        PowerSupplyState::Disconnected => {
            (PowerSupplyStateValue::Disconnected, "Supply: Disconnected")
        }
        PowerSupplyState::InvalidDemand => (
            PowerSupplyStateValue::InvalidDemand,
            "Supply: Invalid demand",
        ),
    }
}

/// Wait for complete layout, never accept a clipped portion of the Text node.
fn widget(world: &World, entity: Entity, expected: &str, primary: Entity) -> Option<Value> {
    let viewport = Vec2::new(1280.0, 720.0);
    let rect =
        crate::assets::building_asset_set::observed_ui_visible_rect(world, entity, viewport)?;
    let computed = world.get::<ComputedNode>(entity)?;
    let transform = world.get::<UiGlobalTransform>(entity)?;
    let camera = camera_binding(
        world,
        world.get::<ComputedUiTargetCamera>(entity)?.get(),
        primary,
    )?;
    let color = world.get::<TextColor>(entity)?.0.to_srgba().to_f32_array();
    if !color
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        || color[3] <= 0.0
    {
        return None;
    }
    let half = computed.size() * 0.5;
    let full = [
        transform.translation.x - half.x,
        transform.translation.y - half.y,
        transform.translation.x + half.x,
        transform.translation.y + half.y,
    ];
    if world.get::<Text>(entity)?.0 != expected || !fully_visible(full, rect) {
        return None;
    }
    Some(
        json!({"entity": entity.to_bits(), "text": expected, "rect_physical": rect,
        "rect_logical": rect, "color_srgba": color, "camera": camera, "visible": true, "fully_visible": true}),
    )
}

fn fully_visible(full: [f32; 4], visible: [f32; 4]) -> bool {
    full.iter()
        .zip(visible)
        .all(|(a, b)| a.is_finite() && b.is_finite() && (*a - b).abs() <= 0.01)
}

fn camera_binding(world: &World, entity: Option<Entity>, primary: Entity) -> Option<Value> {
    use bevy::{camera::RenderTarget, window::WindowRef};
    let entity = entity?;
    let camera = world.get::<Camera>(entity)?;
    let target = world.get::<RenderTarget>(entity)?;
    let primary_target = match target {
        RenderTarget::Window(WindowRef::Primary) => true,
        RenderTarget::Window(WindowRef::Entity(target)) => *target == primary,
        _ => false,
    };
    if !camera.is_active
        || camera.viewport.is_some()
        || !primary_target
        || camera.physical_viewport_size() != Some(UVec2::new(1280, 720))
    {
        return None;
    }
    Some(
        json!({"entity": entity.to_bits(), "window": primary.to_bits(), "target": "owned-primary-window",
        "active": true, "full_viewport": true, "viewport_physical": [1280, 720]}),
    )
}

pub(super) fn snapshot(
    world: &mut World,
    owner: Entity,
    ordinal: u8,
) -> Result<Option<Value>, String> {
    use crate::interface::{selection::SelectedEntity, ui::InfoPanelPinState};
    if !matches!(ordinal, 0 | 2) {
        return Err("Lamp UI ordinal differs".into());
    }
    let mut windows =
        world.query_filtered::<(Entity, &Window), With<bevy::window::PrimaryWindow>>();
    let Ok((window_entity, window)) = windows.single(world) else {
        return Err("Lamp UI primary window differs".into());
    };
    if window.physical_size() != UVec2::new(1280, 720)
        || window.size() != Vec2::new(1280.0, 720.0)
        || window.scale_factor() != 1.0
    {
        return Err("Lamp UI actual DPI/viewport differs".into());
    }
    let scale_factor = window.scale_factor();
    let selected = world.resource::<SelectedEntity>().0;
    let pinned = world.resource::<InfoPanelPinState>().entity;
    let shell = world.resource::<UiShellState>();
    if selected != Some(owner)
        || pinned.is_some()
        || shell.inspected_entity(selected, pinned) != Some(owner)
    {
        return Err("Lamp UI selected/pinned/inspected owner differs".into());
    }
    if shell.page != WorkspacePage::Inspector {
        return Ok(None);
    }
    let Some(model) = &world.resource::<EntityInspectionViewModel>().model else {
        return Ok(None);
    };
    if model.entity != owner || model.header != "Building: OutdoorLamp" {
        return Ok(None);
    }
    let Some(power) = &model.power else {
        return Ok(None);
    };
    let Some(state) = world.get::<PowerSupplyState>(owner).copied() else {
        return Ok(None);
    };
    let expected_on = ordinal == 2;
    if !matches!(
        (expected_on, state),
        (true, PowerSupplyState::Supplied) | (false, PowerSupplyState::Shed { .. })
    ) {
        return Err("Lamp UI supply differs from seeded off/on graph".into());
    }
    let consumer = world
        .get::<PowerConsumer>(owner)
        .ok_or("Lamp UI demand missing")?;
    if !consumer.demand.is_finite() || (consumer.demand - 0.2).abs() > 0.000001 {
        return Err("Lamp UI actual demand differs".into());
    }
    let (expected_state, label) = supply(state);
    let grid = world
        .get::<ConsumesFrom>(owner)
        .ok_or("Lamp UI grid missing")?
        .0;
    if power.supply_state != Some(expected_state)
        || power.demand_watts != Some(consumer.demand)
        || power.grid != Some(grid)
        || power.role != PowerInspectionRole::Consumer
    {
        return Ok(None);
    }
    let nodes = world.resource::<InfoPanelNodes>();
    let (Some(header), Some(power_state)) = (nodes.common.header, nodes.power.power_state) else {
        return Ok(None);
    };
    if header == power_state {
        return Err("Lamp UI header/state nodes alias".into());
    }
    let Some(header) = widget(world, header, &model.header, window_entity) else {
        return Ok(None);
    };
    let Some(power_text) = widget(
        world,
        power_state,
        &format!("{label} / Demand: {:.1}W", consumer.demand),
        window_entity,
    ) else {
        return Ok(None);
    };
    Ok(Some(
        json!({"schema": 1, "scope": "seeded-no-input-rendering-only", "ordinal": ordinal,
        "owner": owner.to_bits(), "selected": owner.to_bits(), "pinned": null, "inspected": owner.to_bits(),
        "model_owner": model.entity.to_bits(), "grid": grid.to_bits(), "kind": "OutdoorLamp", "powered": expected_on,
        "supply_state": format!("{state:?}"), "demand": consumer.demand,
        "header": header, "power_text": power_text, "viewport_physical": [1280, 720],
        "window": window_entity.to_bits(), "scale_factor": scale_factor, "ordinary_input_evidence": false, "natural_supply_evidence": false}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lamp_ui_target_camera_requires_owned_full_primary_output() {
        use bevy::{
            camera::{ComputedCameraValues, RenderTarget, RenderTargetInfo, Viewport},
            window::WindowRef,
        };
        let mut world = World::new();
        let primary = world.spawn_empty().id();
        let foreign = world.spawn_empty().id();
        let camera = world
            .spawn((
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(1280, 720),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
                RenderTarget::Window(WindowRef::Primary),
            ))
            .id();
        assert!(camera_binding(&world, Some(camera), primary).is_some());
        assert!(camera_binding(&world, None, primary).is_none());
        assert!(camera_binding(&world, Some(foreign), primary).is_none());
        world.get_mut::<Camera>(camera).unwrap().is_active = false;
        assert!(camera_binding(&world, Some(camera), primary).is_none());
        world.get_mut::<Camera>(camera).unwrap().is_active = true;
        world.get_mut::<Camera>(camera).unwrap().viewport = Some(Viewport {
            physical_size: UVec2::new(100, 100),
            ..default()
        });
        assert!(camera_binding(&world, Some(camera), primary).is_none());
        world.get_mut::<Camera>(camera).unwrap().viewport = None;
        world
            .entity_mut(camera)
            .insert(RenderTarget::Window(WindowRef::Entity(foreign)));
        assert!(camera_binding(&world, Some(camera), primary).is_none());
        world
            .entity_mut(camera)
            .insert(RenderTarget::Image(Handle::<Image>::default().into()));
        assert!(camera_binding(&world, Some(camera), primary).is_none());
        world
            .entity_mut(camera)
            .insert(RenderTarget::Window(WindowRef::Entity(primary)));
        assert!(camera_binding(&world, Some(camera), primary).is_some());
        world
            .get_mut::<Camera>(camera)
            .unwrap()
            .computed
            .target_info
            .as_mut()
            .unwrap()
            .physical_size
            .x = 640;
        assert!(camera_binding(&world, Some(camera), primary).is_none());
    }

    #[test]
    fn lamp_ui_rejects_clipped_or_nonfinite_text_rect() {
        let full = [10.0, 20.0, 110.0, 40.0];
        assert!(fully_visible(full, full));
        for changed in [
            [11.0, 20.0, 110.0, 40.0],
            [10.0, 20.0, 109.0, 40.0],
            [f32::NAN, 20.0, 110.0, 40.0],
            [10.0, 20.0, f32::INFINITY, 40.0],
        ] {
            assert!(!fully_visible(full, changed));
            assert!(!fully_visible(changed, full));
        }
    }

    #[test]
    fn lamp_supply_observation_distinguishes_all_real_states() {
        for (state, expected) in [
            (PowerSupplyState::Supplied, "Supply: Supplied"),
            (PowerSupplyState::Disconnected, "Supply: Disconnected"),
            (PowerSupplyState::InvalidDemand, "Supply: Invalid demand"),
            (
                PowerSupplyState::Shed {
                    reason: PowerShedReason::InsufficientGeneration,
                },
                "Supply: Shed — insufficient generation",
            ),
            (
                PowerSupplyState::Shed {
                    reason: PowerShedReason::RestoreMargin,
                },
                "Supply: Shed — waiting for restore margin",
            ),
            (
                PowerSupplyState::Shed {
                    reason: PowerShedReason::LegacyGlobalDeficit,
                },
                "Supply: Shed — legacy grid deficit",
            ),
        ] {
            assert_eq!(supply(state).1, expected);
        }
    }
}
