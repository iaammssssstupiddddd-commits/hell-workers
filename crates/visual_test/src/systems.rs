use bevy::camera::RenderTarget;
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite_render::MeshMaterial2d;
use bevy::window::PrimaryWindow;
use hw_core::constants::{
    VIEW_HEIGHT, Z_OFFSET, Z_RTT_COMPOSITE, topdown_rtt_vertical_compensation,
};

use crate::building::{
    TestBuilding, TestBuilding3dHandles, TestBuilding3dVisual, TestBuildingAssets,
    despawn_test_building_at, spawn_test_building,
};
use crate::types::*;

type BtnQuery<'w, 's> = Query<
    'w,
    's,
    (&'static VisualTestAction, &'static Interaction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(SystemParam)]
pub struct BuildingInteractionContext<'w, 's> {
    assets: Option<Res<'w, TestBuildingAssets>>,
    handles_3d: Option<Res<'w, TestBuilding3dHandles>>,
    buildings: Query<'w, 's, (Entity, &'static TestBuilding)>,
    visuals_3d: Query<'w, 's, (Entity, &'static TestBuilding3dVisual)>,
}

/// Apply menu button actions to the top-down building fixture.
pub fn handle_button_interactions(
    q_btns: BtnQuery,
    mut state: ResMut<TestState>,
    mut commands: Commands,
    building: BuildingInteractionContext,
) {
    for (action, interaction) in q_btns.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            VisualTestAction::SetBuildingKind(kind) => state.building_kind = kind,
            VisualTestAction::PlaceOrRemove => {
                let grid = state.building_cursor;
                let occupied = building.buildings.iter().any(|(_, item)| item.grid == grid);
                if occupied {
                    despawn_test_building_at(
                        &mut commands,
                        grid,
                        &building.buildings,
                        &building.visuals_3d,
                    );
                } else if let (Some(assets), Some(handles)) =
                    (building.assets.as_deref(), building.handles_3d.as_deref())
                {
                    spawn_test_building(&mut commands, state.building_kind, grid, assets, handles);
                }
            }
            VisualTestAction::RemoveAllBuildings => {
                for (entity, _) in building.buildings.iter() {
                    commands.entity(entity).despawn();
                }
                for (entity, _) in building.visuals_3d.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

pub fn sync_test_camera3d(q_cam2d: Cam2dQuery, mut q_cam3d: Cam3dSyncQuery) {
    let Ok(cam2d) = q_cam2d.single() else {
        return;
    };
    let scene_z = -cam2d.translation.y;

    for (mut cam3d, mut projection) in &mut q_cam3d {
        *cam3d = Transform::from_xyz(cam2d.translation.x, VIEW_HEIGHT, scene_z + Z_OFFSET)
            .looking_at(Vec3::new(cam2d.translation.x, 0.0, scene_z), Vec3::NEG_Z);
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = cam2d.scale.x;
        }
    }
}

pub fn sync_test_rtt_to_window(
    q_window: Query<Ref<Window>, With<PrimaryWindow>>,
    mut runtime: ResMut<VisualTestRttRuntime>,
    mut images: ResMut<Assets<Image>>,
    mut scene_target: Query<&mut RenderTarget, With<Camera3dRtt>>,
    q_composite: Query<&MeshMaterial2d<LocalRttCompositeMaterial>, With<LocalRttComposite>>,
    mut composite_materials: ResMut<Assets<LocalRttCompositeMaterial>>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    if !window.is_changed() {
        return;
    }

    let physical_size = UVec2::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
    );
    let target_scale_factor = window.scale_factor();
    if runtime.physical_size == physical_size && runtime.target_scale_factor == target_scale_factor
    {
        return;
    }

    runtime.physical_size = physical_size;
    runtime.target_scale_factor = target_scale_factor;
    runtime.scene = create_test_rtt_texture(physical_size, &mut images);

    if let Ok(mut target) = scene_target.single_mut() {
        *target = runtime.scene_target();
    }
    if let Ok(material_handle) = q_composite.single()
        && let Some(mut material) = composite_materials.get_mut(&material_handle.0)
    {
        material.scene_texture = runtime.scene.clone();
        material.params.pixel_size = runtime.pixel_size();
    }
}

fn create_test_rtt_texture(size: UVec2, images: &mut Assets<Image>) -> Handle<Image> {
    images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ))
}

pub fn apply_composite_sprite(
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_composite: Query<&mut Transform, With<LocalRttComposite>>,
) {
    let Ok(mut transform) = q_composite.single_mut() else {
        return;
    };
    let Ok(window) = q_window.single() else {
        return;
    };
    let size = window.size();
    transform.scale = Vec3::new(size.x, size.y * topdown_rtt_vertical_compensation(), 1.0);
    transform.translation.z = Z_RTT_COMPOSITE;
}

/// Scroll the menu without also zooming the world camera.
pub fn handle_panel_scroll(
    q_window: Query<&Window, With<PrimaryWindow>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut q_pan_cam: Query<&mut PanCamera>,
    mut q_scroll: Query<&mut ScrollPosition, With<MenuPanel>>,
    state: Res<TestState>,
) {
    let over_panel = state.menu_visible
        && q_window
            .single()
            .ok()
            .and_then(|window| {
                window
                    .cursor_position()
                    .map(|position| position.x > window.width() - MENU_WIDTH)
            })
            .unwrap_or(false);

    for mut camera in &mut q_pan_cam {
        camera.zoom_speed = if over_panel { 0.0 } else { 0.1 };
    }

    if over_panel && scroll.delta != Vec2::ZERO {
        let delta_px = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * 40.0,
            MouseScrollUnit::Pixel => scroll.delta.y,
        };
        if let Ok(mut position) = q_scroll.single_mut() {
            position.0.y = (position.0.y - delta_px).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::window::WindowResolution;

    #[test]
    fn visual_test_rtt_rebinds_after_resize_and_dpi_change() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<LocalRttCompositeMaterial>()
            .add_systems(Update, sync_test_rtt_to_window);

        let initial_size = UVec2::new(1280, 720);
        let scene = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            create_test_rtt_texture(initial_size, &mut images)
        };
        let runtime = VisualTestRttRuntime {
            physical_size: initial_size,
            target_scale_factor: 1.0,
            scene,
        };
        let initial_scene = runtime.scene.clone();
        let scene_camera = app
            .world_mut()
            .spawn((Camera3dRtt, runtime.scene_target()))
            .id();
        app.insert_resource(runtime);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();

        app.update();
        {
            let mut window = app.world_mut().get_mut::<Window>(window).unwrap();
            window.resolution.set_physical_resolution(1920, 1080);
            window.resolution.set_scale_factor_override(Some(1.5));
        }
        app.update();

        let runtime = app.world().resource::<VisualTestRttRuntime>();
        assert_eq!(runtime.physical_size, UVec2::new(1920, 1080));
        assert_eq!(runtime.target_scale_factor, 1.5);
        assert_ne!(runtime.scene, initial_scene);

        let RenderTarget::Image(target) = app
            .world()
            .get::<RenderTarget>(scene_camera)
            .expect("test camera should keep an image render target")
        else {
            panic!("test camera should keep an image render target");
        };
        assert_eq!(target.scale_factor, 1.5);
    }
}
