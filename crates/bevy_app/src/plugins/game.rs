//! Production App のゲーム固有構成を集約するプラグイン。

use bevy::prelude::*;
#[cfg(feature = "profiling")]
use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    time::{Fixed, TimeUpdateStrategy},
};
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::FixedAuditSeed;

use crate::{
    DamnedSoulPlugin, DebugInstantBuild, DebugVisible,
    plugins::{
        input::InputPlugin,
        interface::InterfacePlugin,
        logic::LogicPlugin,
        messages::MessagesPlugin,
        spatial::SpatialPlugin,
        startup::{PerfScenarioConfig, StartupPlugin},
        visual::VisualPlugin,
    },
    systems::{GameSystemSet, save::SavePlugin, settings::SettingsPlugin},
};
use hw_core::game_state::PlayMode;

/// production App が所有するゲーム側の plugin/resource/system set 構成。
///
/// Window、renderer、platform backend といった実行環境固有の設定は binary shell
/// に残し、この plugin は simulation と presentation の登録だけを一意に所有する。
pub struct HellWorkersGamePlugin {
    perf_config: PerfScenarioConfig,
}

impl HellWorkersGamePlugin {
    pub fn new(perf_config: PerfScenarioConfig) -> Self {
        Self { perf_config }
    }

    /// DefaultPlugins を構築する前に shell が使うログフィルター。
    pub const fn log_filter(&self) -> &'static str {
        if self.perf_config.enabled() {
            "wgpu=error,bevy_app=warn"
        } else {
            "wgpu=error,bevy_app=info"
        }
    }
}

impl Default for HellWorkersGamePlugin {
    fn default() -> Self {
        Self::new(PerfScenarioConfig::default())
    }
}

impl Plugin for HellWorkersGamePlugin {
    fn build(&self, app: &mut App) {
        report_perf_scenario(&self.perf_config);

        let (render3d_visible, render_perf_toggles) = self.perf_config.initial_render_resources();

        #[cfg(feature = "profiling")]
        let fixed_step_audit = self.perf_config.uses_fixed_timesteps();
        #[cfg(feature = "profiling")]
        if fixed_step_audit {
            // Bevy 0.19 guarantees that this advances virtual time by the current
            // fixed timestep and runs FixedMain exactly once per App::update.
            // Normal game systems remain in Update; this audit fixes their
            // Time<Virtual> delta without changing their schedule ownership.
            app.insert_resource(Time::<Fixed>::from_hz(
                self.perf_config.fixed_step_hz() as f64
            ));
            app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
            app.insert_resource(FixedAuditSeed(self.perf_config.master_seed));
        }

        app.insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
            .insert_resource(self.perf_config.clone())
            .insert_resource(render3d_visible)
            .insert_resource(render_perf_toggles)
            .init_resource::<DebugVisible>()
            .init_resource::<DebugInstantBuild>()
            .init_state::<PlayMode>()
            .add_plugins(MessagesPlugin)
            .add_plugins(DamnedSoulPlugin);

        configure_game_system_sets(app);

        // Each parent plugin owns its nested plugins. Adding those children here
        // would make Bevy reject the duplicate registration.
        app.add_plugins(StartupPlugin)
            .add_plugins(InputPlugin)
            .add_plugins(SpatialPlugin)
            .add_plugins(LogicPlugin)
            .add_plugins(VisualPlugin)
            .add_plugins(InterfacePlugin)
            .add_plugins(SettingsPlugin)
            .add_plugins(SavePlugin);

        #[cfg(feature = "profiling")]
        if !fixed_step_audit {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
    }
}

fn report_perf_scenario(perf_config: &PerfScenarioConfig) {
    if perf_config.enabled() {
        eprintln!(
            "PERF_SCENARIO: seed={} workload={} size={} souls={} familiars={} render={} clock={} warmup={}s measure={}s fixed_hz={} fixed_warmup_ticks={} fixed_audit_ticks={} virtual_speed=1.0 output_dir={}",
            perf_config.master_seed,
            perf_config.workload.as_str(),
            perf_config.size.as_str(),
            perf_config.soul_count,
            perf_config.familiar_count,
            perf_config.render_mode.as_str(),
            perf_config.clock_mode_as_str(),
            perf_config.warmup_secs,
            perf_config.measure_secs,
            perf_config.fixed_step_hz(),
            perf_config.fixed_warmup_ticks(),
            perf_config.fixed_audit_ticks(),
            perf_config.output_dir.as_deref().map_or_else(
                || "<default>".to_string(),
                |path| path.display().to_string()
            ),
        );
    }
}

fn configure_game_system_sets(app: &mut App) {
    app.configure_sets(
        Update,
        (
            GameSystemSet::Input,
            GameSystemSet::Spatial.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
            GameSystemSet::Logic.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
            GameSystemSet::Actor.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
            GameSystemSet::Visual,
            GameSystemSet::Interface,
        )
            .chain(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        app::PluginGroup,
        asset::AssetApp,
        core_pipeline::CorePipelinePlugin,
        gilrs::GilrsPlugin,
        gizmos_render::GizmoRenderPlugin,
        image::ImagePlugin,
        pbr::PbrPlugin,
        render::RenderPlugin,
        shader::{Shader, ShaderLoader},
        winit::WinitPlugin,
    };

    #[derive(Default)]
    struct HeadlessShaderAssetsPlugin;

    impl Plugin for HeadlessShaderAssetsPlugin {
        fn build(&self, app: &mut App) {
            app.init_asset::<Shader>()
                .init_asset_loader::<ShaderLoader>();
        }
    }

    #[derive(Resource, Default)]
    struct SystemOrder(Vec<&'static str>);

    fn record_input(mut order: ResMut<SystemOrder>) {
        order.0.push("input");
    }

    fn record_spatial(mut order: ResMut<SystemOrder>) {
        order.0.push("spatial");
    }

    fn record_logic(mut order: ResMut<SystemOrder>) {
        order.0.push("logic");
    }

    fn record_actor(mut order: ResMut<SystemOrder>) {
        order.0.push("actor");
    }

    fn record_visual(mut order: ResMut<SystemOrder>) {
        order.0.push("visual");
    }

    fn record_interface(mut order: ResMut<SystemOrder>) {
        order.0.push("interface");
    }

    #[test]
    fn game_system_sets_keep_the_production_order() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SystemOrder>();
        configure_game_system_sets(&mut app);
        app.add_systems(
            Update,
            (
                record_input.in_set(GameSystemSet::Input),
                record_spatial.in_set(GameSystemSet::Spatial),
                record_logic.in_set(GameSystemSet::Logic),
                record_actor.in_set(GameSystemSet::Actor),
                record_visual.in_set(GameSystemSet::Visual),
                record_interface.in_set(GameSystemSet::Interface),
            ),
        );

        app.update();

        assert_eq!(
            app.world().resource::<SystemOrder>().0,
            ["input", "spatial", "logic", "actor", "visual", "interface"],
        );
    }

    #[test]
    fn production_game_plugin_has_one_owner_for_each_root_plugin() {
        let mut app = App::new();
        // Material plugins load shader libraries during build. Add the shader asset support
        // before ImagePlugin, which normally relies on RenderPlugin to provide it.
        app.add_plugins(
            DefaultPlugins
                .build()
                .disable::<WinitPlugin>()
                .disable::<RenderPlugin>()
                .disable::<GilrsPlugin>()
                .disable::<CorePipelinePlugin>()
                .disable::<GizmoRenderPlugin>()
                .disable::<PbrPlugin>()
                .add_before::<ImagePlugin>(HeadlessShaderAssetsPlugin),
        )
        .add_plugins(HellWorkersGamePlugin::default());

        assert!(app.world().contains_resource::<DebugVisible>());
        assert!(app.world().contains_resource::<DebugInstantBuild>());
        assert!(app.world().contains_resource::<State<PlayMode>>());
        assert_eq!(app.get_added_plugins::<MessagesPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<DamnedSoulPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<StartupPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<InputPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<SpatialPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<LogicPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<VisualPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<InterfacePlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<SettingsPlugin>().len(), 1);
        assert_eq!(app.get_added_plugins::<SavePlugin>().len(), 1);
    }
}
