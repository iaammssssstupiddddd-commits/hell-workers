use super::super::models::{EntityListSnapshot, SoulGender, StressBucket, TaskVisual};
use super::super::spawn::spawn_soul_list_item_entity;
use super::*;

#[derive(Resource)]
struct TestAssets {
    font: Handle<Font>,
    images: [Handle<Image>; 14],
}
impl Default for TestAssets {
    fn default() -> Self {
        let mut images = Assets::<Image>::default();
        Self {
            font: Handle::default(),
            images: std::array::from_fn(|_| images.add(Image::default())),
        }
    }
}
impl UiAssets for TestAssets {
    fn building_preview(&self, _kind: hw_jobs::BuildingType) -> &Handle<Image> {
        &self.images[0]
    }
    fn icon_arrow_down(&self) -> &Handle<Image> {
        &self.images[1]
    }
    fn icon_arrow_right(&self) -> &Handle<Image> {
        &self.images[2]
    }
    fn icon_idle(&self) -> &Handle<Image> {
        &self.images[3]
    }
    fn glow_circle(&self) -> &Handle<Image> {
        &self.images[4]
    }
    fn icon_stress(&self) -> &Handle<Image> {
        &self.images[5]
    }
    fn icon_fatigue(&self) -> &Handle<Image> {
        &self.images[6]
    }
    fn icon_male(&self) -> &Handle<Image> {
        &self.images[7]
    }
    fn icon_female(&self) -> &Handle<Image> {
        &self.images[8]
    }
    fn icon_axe(&self) -> &Handle<Image> {
        &self.images[9]
    }
    fn icon_pick(&self) -> &Handle<Image> {
        &self.images[10]
    }
    fn icon_hammer(&self) -> &Handle<Image> {
        &self.images[11]
    }
    fn icon_haul(&self) -> &Handle<Image> {
        &self.images[12]
    }
    fn icon_bone_small(&self) -> &Handle<Image> {
        &self.images[13]
    }
    fn font_ui(&self) -> &Handle<Font> {
        &self.font
    }
    fn font_familiar(&self) -> &Handle<Font> {
        &self.font
    }
    fn font_soul_name(&self) -> &Handle<Font> {
        &self.font
    }
}
fn sync(
    assets: Res<TestAssets>,
    theme: Res<UiTheme>,
    vm: Res<EntityListViewModel>,
    index: Res<EntityListNodeIndex>,
    mut queries: EntityListValueQueries,
) {
    sync_entity_list_values(&*assets, &theme, &vm, &index, &mut queries);
}
#[derive(Debug, PartialEq)]
struct Values {
    texts: Vec<(String, Color, FontWeight)>,
    images: Vec<(Handle<Image>, Color)>,
}
fn values(world: &World, nodes: SoulRowNodes) -> Values {
    Values {
        texts: [
            nodes.name_text,
            nodes.fatigue_text,
            nodes.stress_text,
            nodes.dream_text,
            nodes.task_label,
        ]
        .map(|entity| {
            (
                world.get::<Text>(entity).unwrap().0.clone(),
                world.get::<TextColor>(entity).unwrap().0,
                world.get::<TextFont>(entity).unwrap().weight,
            )
        })
        .into(),
        images: [nodes.gender_icon, nodes.task_icon]
            .map(|entity| {
                let image = world.get::<ImageNode>(entity).unwrap();
                (image.image.clone(), image.color)
            })
            .into(),
    }
}
fn assert_no_changes<T: Component>(world: &mut World) {
    let mut query = world.query_filtered::<Entity, Changed<T>>();
    assert_eq!(
        query.iter(world).count(),
        0,
        "{}",
        std::any::type_name::<T>()
    );
}
#[test]
fn soul_row_named_nodes_match_fresh_spawn_and_skip_equal_updates_test() {
    let tasks = [
        TaskVisual::Idle,
        TaskVisual::Chop,
        TaskVisual::Mine,
        TaskVisual::GatherDefault,
        TaskVisual::Haul,
        TaskVisual::Build,
        TaskVisual::HaulToBlueprint,
        TaskVisual::Water,
        TaskVisual::GeneratePower,
        TaskVisual::Deconstruct,
        TaskVisual::Move,
        TaskVisual::Refine,
        TaskVisual::CollectBone,
    ];
    for (i, task) in tasks.into_iter().enumerate() {
        let mut app = App::new();
        let assets = TestAssets::default();
        let theme = UiTheme::default();
        let parent = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        let initial = SoulRowViewModel {
            entity: target,
            name: "Before".into(),
            gender: SoulGender::Male,
            fatigue_text: "1".into(),
            stress_text: "2".into(),
            stress_bucket: StressBucket::Low,
            dream_text: "3".into(),
            dream_empty: false,
            task_visual: TaskVisual::Idle,
        };
        let updated = SoulRowViewModel {
            entity: target,
            name: "After".into(),
            gender: if i % 2 == 0 {
                SoulGender::Female
            } else {
                SoulGender::Male
            },
            fatigue_text: "42".into(),
            stress_text: "73".into(),
            stress_bucket: [StressBucket::Low, StressBucket::Medium, StressBucket::High][i % 3],
            dream_text: "0".into(),
            dream_empty: i % 2 == 0,
            task_visual: task,
        };
        let row = spawn_soul_list_item_entity(
            &mut app.world_mut().commands(),
            parent,
            &initial,
            &assets,
            0.0,
            &theme,
        );
        let fresh = spawn_soul_list_item_entity(
            &mut app.world_mut().commands(),
            parent,
            &updated,
            &assets,
            0.0,
            &theme,
        );
        app.world_mut().flush();
        let nodes = *app.world().get::<SoulRowNodes>(row).unwrap();
        let expected = values(
            app.world(),
            *app.world().get::<SoulRowNodes>(fresh).unwrap(),
        );
        // Capture the independent fresh-spawn oracle before value synchronization.
        app.world_mut().entity_mut(fresh).despawn();
        let children: Vec<_> = app
            .world()
            .get::<Children>(row)
            .unwrap()
            .iter()
            .rev()
            .collect();
        app.world_mut().entity_mut(row).replace_children(&children);
        app.insert_resource(assets)
            .insert_resource(theme)
            .init_resource::<EntityListNodeIndex>()
            .insert_resource(EntityListViewModel {
                current: EntityListSnapshot {
                    unassigned: vec![updated],
                    ..default()
                },
                ..default()
            })
            .add_systems(Update, sync);
        app.update();
        assert_eq!(*app.world().get::<SoulRowNodes>(row).unwrap(), nodes);
        assert_eq!(values(app.world(), nodes), expected);
        assert_eq!(
            app.world().get::<Text>(nodes.task_label).unwrap().0,
            task.label()
        );
        app.world_mut().clear_trackers();
        app.update();
        assert_no_changes::<Text>(app.world_mut());
        assert_no_changes::<TextFont>(app.world_mut());
        assert_no_changes::<TextColor>(app.world_mut());
        assert_no_changes::<ImageNode>(app.world_mut());
        app.world_mut().entity_mut(row).despawn();
        for node in [
            nodes.gender_icon,
            nodes.name_text,
            nodes.fatigue_text,
            nodes.stress_text,
            nodes.dream_text,
            nodes.task_icon,
            nodes.task_label,
        ] {
            assert!(app.world().get_entity(node).is_err());
        }
    }
}
