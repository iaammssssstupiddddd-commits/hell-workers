use hw_core::constants::Z_SELECTION;
use crate::systems::visual::task_area_visual::TaskAreaMaterial;
use crate::systems::world::zones::{AreaBounds, Site, Yard};
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

#[derive(Component)]
pub struct SiteYardBoundaryVisual;

const SITE_BOUNDARY_COLOR: LinearRgba = LinearRgba::new(0.62, 0.58, 0.49, 0.85);
const YARD_BOUNDARY_COLOR: LinearRgba = LinearRgba::new(0.28, 0.88, 0.95, 0.85);

pub fn sync_site_yard_boundaries_system(
    mut commands: Commands,
    q_existing: Query<Entity, With<SiteYardBoundaryVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TaskAreaMaterial>>,
    q_sites: Query<&Site>,
    q_yards: Query<&Yard>,
    q_sites_changed: Query<Entity, (With<Site>, Changed<Site>)>,
    q_yards_changed: Query<Entity, (With<Yard>, Changed<Yard>)>,
) {
    if q_sites_changed.is_empty() && q_yards_changed.is_empty() {
        return;
    }

    for visual in q_existing.iter() {
        commands.entity(visual).despawn();
    }

    for site in q_sites.iter() {
        spawn_boundary_visual(
            &mut commands,
            &mut meshes,
            &mut materials,
            &site.bounds(),
            SITE_BOUNDARY_COLOR,
        );
    }

    for yard in q_yards.iter() {
        spawn_boundary_visual(
            &mut commands,
            &mut meshes,
            &mut materials,
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
        Transform::from_translation(center.extend(Z_SELECTION + 0.05))
            .with_scale(size.extend(1.0)),
        Visibility::Visible,
        Name::new("SiteYardBoundary"),
    ));
}
