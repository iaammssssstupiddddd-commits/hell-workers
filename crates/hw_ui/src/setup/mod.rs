//! UIセットアップモジュール
//!
//! UIの初期構造を構築します。

mod bottom_bar;
mod dialogs;
mod entity_list;
mod help_panel;
mod panels;
mod pause_menu;
mod root;
mod settings_panel;
mod submenus;
mod time_control;

use bevy::prelude::{Font, Handle, Image};

pub trait UiAssets {
    fn font_ui(&self) -> &Handle<Font>;
    fn font_familiar(&self) -> &Handle<Font>;
    fn font_soul_name(&self) -> &Handle<Font>;
    fn icon_arrow_down(&self) -> &Handle<Image>;
    fn icon_arrow_right(&self) -> &Handle<Image>;
    fn icon_idle(&self) -> &Handle<Image>;
    fn glow_circle(&self) -> &Handle<Image>;
    fn icon_stress(&self) -> &Handle<Image>;
    fn icon_fatigue(&self) -> &Handle<Image>;
    fn icon_male(&self) -> &Handle<Image>;
    fn icon_female(&self) -> &Handle<Image>;
    fn icon_axe(&self) -> &Handle<Image>;
    fn icon_pick(&self) -> &Handle<Image>;
    fn icon_hammer(&self) -> &Handle<Image>;
    fn icon_haul(&self) -> &Handle<Image>;
    fn icon_bone_small(&self) -> &Handle<Image>;
}

pub use root::{SetupUiParams, setup_ui};
pub use settings_panel::{SettingsPanelInitial, spawn_settings_panel};

#[cfg(test)]
pub(super) mod test_support {
    use super::UiAssets;
    use crate::help::{HelpPanelChrome, HelpPanelCopy, HelpPanelCopySpec, HelpShortcutPair};
    use bevy::prelude::{Font, Handle, Image};

    #[derive(Default)]
    pub struct TestAssets {
        font: Handle<Font>,
        image: Handle<Image>,
    }

    impl UiAssets for TestAssets {
        fn font_ui(&self) -> &Handle<Font> {
            &self.font
        }

        fn font_familiar(&self) -> &Handle<Font> {
            &self.font
        }

        fn font_soul_name(&self) -> &Handle<Font> {
            &self.font
        }

        fn icon_arrow_down(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_arrow_right(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_idle(&self) -> &Handle<Image> {
            &self.image
        }

        fn glow_circle(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_stress(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_fatigue(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_male(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_female(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_axe(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_pick(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_hammer(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_haul(&self) -> &Handle<Image> {
            &self.image
        }

        fn icon_bone_small(&self) -> &Handle<Image> {
            &self.image
        }
    }

    pub fn sentinel_help_chrome() -> HelpPanelChrome {
        HelpPanelChrome::new(
            HelpPanelCopy::new(HelpPanelCopySpec {
                launcher_label: "Injected Help",
                launcher_tooltip: "Injected Help Tooltip",
                panel_title: "Injected Help Title",
                close_label: "Injected Close",
                topic_navigation_label: "Injected Topics",
                page_navigation_label: "Injected Pages",
                document_bounds_label: "Injected Bounds",
                shortcut_label: "Injected Shortcut",
            }),
            "Ctrl+F1",
            "Ctrl+F1 / Esc",
            HelpShortcutPair::new("PrevTopic", "NextTopic"),
            HelpShortcutPair::new("PrevPage", "NextPage"),
            HelpShortcutPair::new("DocumentStart", "DocumentEnd"),
        )
    }
}
