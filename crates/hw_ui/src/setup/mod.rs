//! UIセットアップモジュール
//!
//! UIの初期構造を構築します。

mod bottom_bar;
mod dialogs;
mod entity_list;
mod panels;
mod root;
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

pub use root::setup_ui;
