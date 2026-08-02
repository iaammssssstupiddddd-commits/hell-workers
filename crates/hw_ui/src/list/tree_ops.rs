// ツリー操作（clear_children 等）

use bevy::prelude::*;

pub fn clear_children(commands: &mut Commands, q_children: &Query<&Children>, parent: Entity) {
    if let Ok(children) = q_children.get(parent) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct Root;

    fn clear_root_children(
        mut commands: Commands,
        q_roots: Query<Entity, With<Root>>,
        q_children: Query<&Children>,
    ) {
        let root = q_roots.single().expect("one root");
        clear_children(&mut commands, &q_children, root);
    }

    #[test]
    fn clears_nested_children_without_queueing_duplicate_descendant_despawns() {
        let mut app = App::new();
        app.set_error_handler(bevy::ecs::error::panic);
        app.add_systems(Update, clear_root_children);

        let root = app.world_mut().spawn(Root).id();
        let child = app.world_mut().spawn(ChildOf(root)).id();
        let grandchild = app.world_mut().spawn(ChildOf(child)).id();

        app.update();

        assert!(app.world().get_entity(root).is_ok());
        assert!(app.world().get_entity(child).is_err());
        assert!(app.world().get_entity(grandchild).is_err());
    }
}
