//! Passive M5 sprite observations. A non-white tint is evidence, not repaired here.

use super::*;
use crate::assets::building_asset_set::BuildingAssetDescriptor;
use bevy::{camera::visibility::RenderLayers, sprite::Anchor};

pub(super) type ForegroundQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ChildOf,
        &'static Sprite,
        &'static Transform,
        &'static GlobalTransform,
        Option<&'static Anchor>,
        Option<&'static RenderLayers>,
        &'static ViewVisibility,
    ),
    With<hw_visual::layer::VisualLayerKind>,
>;

pub(super) fn unchanged(initial: Option<&Value>, current: &Value) -> Result<(), String> {
    let Some(initial) = initial else {
        return Ok(());
    };
    let sprites = |value: &Value| {
        value["records"].as_array().map(|records| {
            records
                .iter()
                .filter(|record| record.get("foreground").is_some())
                .cloned()
                .collect::<Vec<_>>()
        })
    };
    let before = sprites(initial);
    let after = sprites(current);
    if initial["foreground_schema"] != json!(1)
        || current["foreground_schema"] != json!(1)
        || before.as_ref().is_none_or(|records| records.len() != 4)
        || after != before
    {
        return Err("foreground measured sprite changed after readiness".into());
    }
    Ok(())
}

fn pose(value: &Transform) -> Value {
    json!({"translation": value.translation.to_array(), "rotation": value.rotation.to_array(),
        "scale": value.scale.to_array()})
}

fn measured_values(
    sprite: &Sprite,
    owner_pose: &Transform,
    local: &Transform,
    global: &GlobalTransform,
) -> Result<Value, String> {
    let expected_global = GlobalTransform::from(*owner_pose).mul_transform(*local);
    let color = sprite.color.to_srgba().to_f32_array();
    if color.iter().any(|value| !value.is_finite())
        || !global
            .to_matrix()
            .abs_diff_eq(expected_global.to_matrix(), 0.001)
    {
        return Err("foreground non-finite color or parent transform differs".into());
    }
    Ok(
        json!({"color_srgba": color, "canvas_wu": sprite.custom_size.map(|size| size.to_array()),
        "owner_pose": pose(owner_pose), "local_pose": pose(local),
        "global_pose": pose(&global.compute_transform()),
        "global_matrix": global.to_matrix().to_cols_array(),
        "parent_local_matrix": expected_global.to_matrix().to_cols_array()}),
    )
}

pub(super) fn snapshot(
    params: &InspectParams,
    spec: &Specimen,
    owner: Entity,
    descriptor: &BuildingAssetDescriptor,
) -> Result<Value, String> {
    let observations = params
        .foreground
        .iter()
        .filter(|(_, parent, ..)| parent.parent() == owner)
        .collect::<Vec<_>>();
    let [(entity, parent, sprite, local, global, anchor, layers, visible)] =
        observations.as_slice()
    else {
        return Err("foreground requires exactly one owned sprite".into());
    };
    let (_, owner_pose, ..) = params
        .buildings
        .get(owner)
        .map_err(|_| "foreground owner disappeared")?;
    let role = if spec.kind == BuildingType::OutdoorLamp && spec.quarter() >= 2 {
        "world_on"
    } else {
        &descriptor.manifest.world_preview.image_role
    };
    let artifact_role = format!("image:{role}");
    let artifacts = descriptor
        .manifest
        .artifacts
        .iter()
        .filter(|artifact| artifact.role == artifact_role)
        .collect::<Vec<_>>();
    let [artifact] = artifacts.as_slice() else {
        return Err("foreground requires one exact image artifact".into());
    };
    // Preserve the actual color/pose instead of normalizing a suspected tint or Z.
    if descriptor.image(role) != Some(&sprite.image) {
        return Err("foreground image differs".into());
    }
    let mut value = measured_values(sprite, owner_pose, local, global)?;
    value.as_object_mut().expect("measured object").extend(
        json!({"schema": 1, "owner": owner.to_bits(), "entity": entity.to_bits(),
        "parent": parent.parent().to_bits(), "identity": descriptor.manifest.identity,
        "descriptor": descriptor.manifest.world_preview, "image_role": role, "artifact": artifact,
        "image_handle": format!("{:?}", sprite.image.id()), "image_matches": true,
        "anchor": anchor.map_or(Vec2::ZERO, |anchor| anchor.0).to_array(),
        "layers": layers.map_or_else(|| vec![0], |layers| layers.iter().collect::<Vec<_>>()),
        "visible": visible.get()})
        .as_object()
        .expect("metadata object")
        .clone(),
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passive_sprite_drift_is_rejected_not_repaired() {
        let initial = json!({"foreground_schema": 1, "records": (0..4).map(|owner|
            json!({"owner": owner, "foreground": {"color_srgba": [0.2, 0.8, 0.1, 1.0]}})).collect::<Vec<_>>()});
        assert!(unchanged(None, &initial).is_ok());
        assert!(unchanged(Some(&initial), &initial).is_ok());
        let mut changed = initial.clone();
        changed["records"][0]["foreground"]["color_srgba"] = json!([1.0, 1.0, 1.0, 1.0]);
        assert!(unchanged(Some(&initial), &changed).is_err());
        assert_ne!(initial, changed);
        changed["records"] = json!([]);
        assert!(unchanged(Some(&initial), &changed).is_err());
    }

    #[test]
    fn passive_sprite_snapshot_keeps_color_alpha_and_parent_local_pose() {
        let sprite = Sprite {
            color: Color::srgba(0.2, 0.8, 0.1, 0.4),
            custom_size: Some(Vec2::splat(64.0)),
            ..default()
        };
        let owner = Transform::from_xyz(32.0, 64.0, 0.12);
        let local = Transform::from_xyz(0.0, 0.0, 0.01);
        let global = GlobalTransform::from(owner).mul_transform(local);
        let observed = measured_values(&sprite, &owner, &local, &global).unwrap();
        assert_eq!(
            observed["color_srgba"],
            json!(sprite.color.to_srgba().to_f32_array())
        );
        assert_eq!(sprite.color, Color::srgba(0.2, 0.8, 0.1, 0.4));
        assert_eq!(
            observed["owner_pose"]["translation"],
            json!(owner.translation.to_array())
        );
        assert_eq!(observed["global_matrix"], observed["parent_local_matrix"]);
        assert!(measured_values(&sprite, &owner, &local, &GlobalTransform::IDENTITY).is_err());
        let invalid = Sprite {
            color: Color::srgba(f32::NAN, 1.0, 1.0, 1.0),
            ..default()
        };
        assert!(measured_values(&invalid, &owner, &local, &global).is_err());
    }
}
