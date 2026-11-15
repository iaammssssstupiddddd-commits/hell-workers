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
    boundary_mesh: Local<'s, Option<Handle<Mesh>>>,
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

    if p.boundary_mesh.is_none() {
        let mesh = p.meshes.add(Rectangle::default().mesh());
        *p.boundary_mesh = Some(mesh);
    }
    let mesh = p
        .boundary_mesh
        .as_ref()
        .expect("boundary mesh initialized")
        .clone();

    for visual in p.q_existing.iter() {
        commands.entity(visual).despawn();
    }

    for site in p.q_sites.iter() {
        spawn_boundary_visual(
            &mut commands,
            &mesh,
            &mut p.materials,
            &site.bounds(),
            SITE_BOUNDARY_COLOR,
        );
    }

    for yard in p.q_yards.iter() {
        spawn_boundary_visual(
            &mut commands,
            &mesh,
            &mut p.materials,
            &yard.bounds(),
            YARD_BOUNDARY_COLOR,
        );
    }
}

fn spawn_boundary_visual(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    materials: &mut Assets<TaskAreaMaterial>,
    area: &AreaBounds,
    color: LinearRgba,
) {
    let size = area.size();
    let center = area.center();
    commands.spawn((
        SiteYardBoundaryVisual,
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(TaskAreaMaterial {
            color,
            size,
            state: 0,
        })),
        Transform::from_translation(center.extend(Z_SELECTION + 0.05)).with_scale(size.extend(1.0)),
        Visibility::Visible,
        Name::new("SiteYardBoundary"),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_geometry_is_shared_across_owners_and_rebuilds_without_changing_shape_or_material() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<TaskAreaMaterial>()
            .add_systems(Update, sync_site_yard_boundaries_system);
        let site = app
            .world_mut()
            .spawn(Site {
                min: Vec2::ZERO,
                max: Vec2::new(10., 20.),
            })
            .id();
        let yard = app
            .world_mut()
            .spawn(Yard {
                min: Vec2::new(30., 40.),
                max: Vec2::new(50., 70.),
            })
            .id();
        app.update();
        let rows = app.world_mut().query_filtered::<(&Mesh2d, &MeshMaterial2d<TaskAreaMaterial>, &Transform), With<SiteYardBoundaryVisual>>()
            .iter(app.world()).map(|(m, mat, t)| (m.0.id(), mat.0.id(), *t)).collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, rows[1].0);
        assert_ne!(rows[0].1, rows[1].1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        for (_, material, transform) in &rows {
            let material = app
                .world()
                .resource::<Assets<TaskAreaMaterial>>()
                .get(*material)
                .unwrap();
            assert_eq!(material.size, transform.scale.truncate());
            assert_eq!(material.state, 0);
            assert!([SITE_BOUNDARY_COLOR, YARD_BOUNDARY_COLOR].contains(&material.color));
        }
        app.world_mut().get_mut::<Yard>(yard).unwrap().max += Vec2::splat(20.);
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        assert!(
            app.world_mut()
                .query_filtered::<&Mesh2d, With<SiteYardBoundaryVisual>>()
                .iter(app.world())
                .all(|m| m.0.id() == rows[0].0)
        );
        app.world_mut().despawn(site);
    }
}
