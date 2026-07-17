use super::*;

/// 3D表示ボタンのラベルと色を Render3dVisible に合わせて更新
pub fn update_render3d_button_visual_system(
    render3d: Res<crate::Render3dVisible>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRender3dButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !render3d.is_changed() {
        return;
    }
    for (children, mut bg, mut border) in q_button.iter_mut() {
        if render3d.0 {
            *bg = BackgroundColor(Color::srgb(0.15, 0.35, 0.15));
            *border = BorderColor::all(Color::srgb(0.35, 0.55, 0.35));
        } else {
            *bg = BackgroundColor(Color::srgb(0.35, 0.15, 0.15));
            *border = BorderColor::all(Color::srgb(0.55, 0.35, 0.35));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if render3d.0 {
                    "3D: ON".to_string()
                } else {
                    "3D: OFF".to_string()
                };
            }
        }
    }
}

/// LOD インジケーターテキストを毎フレーム更新する
pub fn update_lod_indicator_system(
    metrics: Res<TerrainLodMetrics>,
    state: Res<TerrainLodState>,
    mut q_text: Query<&mut Text, With<LodIndicatorText>>,
) {
    let level = match state.level {
        LodLevel::Lod0 => "0",
        LodLevel::Lod1 => "1",
        LodLevel::Lod1Lite => "1L",
        LodLevel::Lod2 => "2",
    };
    let new_text = format!("LOD:{} rtt:{:.1}px", level, metrics.tile_rtt_px);
    for mut text in q_text.iter_mut() {
        text.0 = new_text.clone();
    }
}

/// RtT 品質と固定費トグルの状態を DevPanel に表示する。
pub fn update_render_perf_status_system(
    quality: Res<QualitySettings>,
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_text: Query<&mut Text, With<RenderPerfStatusText>>,
) {
    if !quality.is_changed() && !perf_toggles.is_changed() {
        return;
    }

    let rtt = match quality.rtt {
        RttQualityPreset::High => "H",
        RttQualityPreset::Medium => "M",
        RttQualityPreset::Low => "L",
    };
    let mask = if perf_toggles.soul_mask_enabled {
        "ON"
    } else {
        "OFF"
    };
    let light = if perf_toggles.directional_light_enabled {
        "ON"
    } else {
        "OFF"
    };
    let light2 = if perf_toggles.extra_directional_light_enabled {
        "ON"
    } else {
        "OFF"
    };
    let terrain = if perf_toggles.terrain_enabled {
        "ON"
    } else {
        "OFF"
    };
    let scene_objects = if perf_toggles.scene_objects_enabled {
        "ON"
    } else {
        "OFF"
    };
    let text = format!(
        "RTT:{rtt} Mask:{mask} Light:{light} Light2:{light2} Terrain:{terrain} Objs:{scene_objects}"
    );

    for mut label in q_text.iter_mut() {
        label.0 = text.clone();
    }
}

/// Soul mask ボタンのラベルと色を同期する。
pub fn update_soul_mask_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleSoulMaskButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.soul_mask_enabled {
            *bg = BackgroundColor(Color::srgb(0.30, 0.24, 0.08));
            *border = BorderColor::all(Color::srgb(0.55, 0.45, 0.18));
        } else {
            *bg = BackgroundColor(Color::srgb(0.18, 0.12, 0.08));
            *border = BorderColor::all(Color::srgb(0.40, 0.26, 0.18));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.soul_mask_enabled {
                    "Mask: ON".to_string()
                } else {
                    "Mask: OFF".to_string()
                };
            }
        }
    }
}

/// RtT light ボタンのラベルと色を同期する。
pub fn update_rtt_light_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttLightButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.directional_light_enabled {
            *bg = BackgroundColor(Color::srgb(0.20, 0.22, 0.08));
            *border = BorderColor::all(Color::srgb(0.42, 0.48, 0.18));
        } else {
            *bg = BackgroundColor(Color::srgb(0.12, 0.12, 0.08));
            *border = BorderColor::all(Color::srgb(0.28, 0.28, 0.18));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.directional_light_enabled {
                    "Light: ON".to_string()
                } else {
                    "Light: OFF".to_string()
                };
            }
        }
    }
}

/// 追加 RtT light ボタンのラベルと色を同期する。
pub fn update_rtt_extra_light_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttExtraLightButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.extra_directional_light_enabled {
            *bg = BackgroundColor(Color::srgb(0.20, 0.14, 0.30));
            *border = BorderColor::all(Color::srgb(0.42, 0.32, 0.56));
        } else {
            *bg = BackgroundColor(Color::srgb(0.12, 0.10, 0.18));
            *border = BorderColor::all(Color::srgb(0.28, 0.24, 0.42));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.extra_directional_light_enabled {
                    "Light2: ON".to_string()
                } else {
                    "Light2: OFF".to_string()
                };
            }
        }
    }
}

/// RtT terrain ボタンのラベルと色を同期する。
pub fn update_rtt_terrain_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttTerrainButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.terrain_enabled {
            *bg = BackgroundColor(Color::srgb(0.12, 0.20, 0.14));
            *border = BorderColor::all(Color::srgb(0.24, 0.42, 0.30));
        } else {
            *bg = BackgroundColor(Color::srgb(0.10, 0.12, 0.10));
            *border = BorderColor::all(Color::srgb(0.22, 0.26, 0.22));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.terrain_enabled {
                    "Terrain: ON".to_string()
                } else {
                    "Terrain: OFF".to_string()
                };
            }
        }
    }
}

/// RtT scene object ボタンのラベルと色を同期する。
pub fn update_rtt_scene_objects_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttSceneObjectsButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.scene_objects_enabled {
            *bg = BackgroundColor(Color::srgb(0.12, 0.16, 0.22));
            *border = BorderColor::all(Color::srgb(0.24, 0.34, 0.44));
        } else {
            *bg = BackgroundColor(Color::srgb(0.10, 0.10, 0.14));
            *border = BorderColor::all(Color::srgb(0.22, 0.22, 0.30));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.scene_objects_enabled {
                    "Objs: ON".to_string()
                } else {
                    "Objs: OFF".to_string()
                };
            }
        }
    }
}

/// 即時ビルドボタンのラベルと色を DebugInstantBuild に合わせて更新
pub fn update_instant_build_button_visual_system(
    instant_build: Res<crate::DebugInstantBuild>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<InstantBuildButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !instant_build.is_changed() {
        return;
    }
    for (children, mut bg, mut border) in q_button.iter_mut() {
        if instant_build.0 {
            *bg = BackgroundColor(Color::srgb(0.35, 0.20, 0.05));
            *border = BorderColor::all(Color::srgb(0.60, 0.35, 0.10));
        } else {
            *bg = BackgroundColor(Color::srgb(0.25, 0.25, 0.25));
            *border = BorderColor::all(Color::srgb(0.45, 0.45, 0.45));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if instant_build.0 {
                    "IBuild: ON".to_string()
                } else {
                    "IBuild: OFF".to_string()
                };
            }
        }
    }
}
