mod components;
mod systems;

pub use components::{
    PlantTreeVisualPhase, PlantTreeVisualState, PlantTreeMagicCircle, PlantTreeLifeSpark,
};
pub use systems::{
    setup_plant_tree_visual_state_system, update_plant_tree_magic_circle_system,
    update_plant_tree_growth_system, update_plant_tree_life_spark_system,
};
