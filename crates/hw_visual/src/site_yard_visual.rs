use crate::task_area_visual::TaskAreaMaterial;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;
use hw_core::constants::Z_SELECTION;
use hw_world::zones::{AreaBounds, Site, Yard};

#[derive(Component)]
pub struct SiteYardBoundaryVisual;

const SITE_BOUNDARY_COLOR: LinearRgba = LinearRgba::new(0.62, 0.58, 0.49, 0.85);
const YARD_BOUNDARY_COLOR: LinearRgba = LinearRgba::new(0.28, 0.88, 0.95, 0.85);

#[derive(SystemParam)]
pub struct SiteYardParams<'w, 's> {
    q_existing: Query<'w, 's, Entity, With<SiteYardBoundaryVisual>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<TaskAreaMaterial>>,
    q_sites: Query<'w, 's, &'static Site>,
    q_yards: Query<'w, 's, &'static Yard>,
    q_sites_changed: Query<'w, 's, Entity, (With<Site>, Changed<Site>)>,
    q_yards_changed: Query<'w, 's, Entity, (With<Yard>, Changed<Yard>)>,
}

pub fn sync_site_yard_boundaries_system(mut commands: Commands, mut p: SiteYardParams) {
    if p.q_sites_changed.is_empty() && p.q_yards_changed.is_empty() {
        return;
    }

    for visual in p.q_existing.iter() {
        commands.entity(visual).despawn();
    }

    for site in p.q_sites.iter() {
        spawn_boundary_visual(
            &mut commands,
            &mut p.meshes,
            &mut p.materials,
            &site.bounds(),
            SITE_BOUNDARY_COLOR,
        );
    }

    for yard in p.q_yards.iter() {
        spawn_boundary_visual(
            &mut commands,
            &mut p.meshes,
            &mut p.materials,
            &yard.bounds(),
            YARD_BOUNDARY_COLOR,
        );
    }
}

fn spawn_boundary_visual(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<TaskAreaMaterial>,
    area: &AreaBounds,
    color: LinearRgba,
) {
    let size = area.size();
    let center = area.center();
    commands.spawn((
        SiteYardBoundaryVisual,
        Mesh2d(meshes.add(Rectangle::default().mesh())),
        MeshMaterial2d(materials.add(TaskAreaMaterial {
            color,
            size,
            time: 0.0,
            state: 0,
        })),
        Transform::from_translation(center.extend(Z_SELECTION + 0.05)).with_scale(size.extend(1.0)),
        Visibility::Visible,
        Name::new("SiteYardBoundary"),
    ));
}
