//! Read-only exact-part evidence for explicit seeded SoulSpa mask pages.
use super::*;
use crate::assets::building_asset_set::{BuildingAssetDescriptor, BuildingPartMaterial};

pub(super) fn unchanged(initial: Option<&Value>, current: &Value) -> Result<(), String> {
    if initial.is_some_and(|initial| initial != current) {
        return Err("Spa page snapshot changed after readiness".into());
    }
    Ok(())
}

pub(super) fn observe_part(
    params: &InspectParams,
    specimen: &Specimen,
    set: &BuildingAssetDescriptor,
    owner: Entity,
    entity: Entity,
) -> Result<Value, String> {
    observe(params, specimen, set, owner, entity, true)
}

pub(super) fn observe_resident(
    params: &InspectParams,
    specimen: &Specimen,
    set: &BuildingAssetDescriptor,
    owner: Entity,
    entity: Entity,
) -> Result<Value, String> {
    // M6 validates the actual visibility separately, including fixed groups
    // outside a group gallery viewport. Keep mask-page visibility strict.
    observe(params, specimen, set, owner, entity, false)
}

fn observe(
    params: &InspectParams,
    specimen: &Specimen,
    set: &BuildingAssetDescriptor,
    owner: Entity,
    entity: Entity,
    require_visible: bool,
) -> Result<Value, String> {
    let name = params
        .part_names
        .get(entity)
        .map_err(|_| "Spa part name missing")?
        .as_str();
    let part = set
        .manifest
        .parts
        .iter()
        .find(|part| part.name == name)
        .ok_or("Spa part name not in exact descriptor")?;
    let active = if name == "body" {
        false
    } else {
        let slot = name
            .strip_prefix("slot")
            .and_then(|slot| slot.parse::<u8>().ok())
            .filter(|slot| *slot < 4)
            .ok_or("unknown Spa slot name")?;
        specimen.spa_mask() & (1 << slot) != 0
    };
    let (_, _, parent, mesh, material, visible) = params
        .visuals
        .get(entity)
        .map_err(|_| "Spa indexed visual missing")?;
    let parent = parent.ok_or("Spa indexed part parent missing")?.parent();
    let (_, visual, root) = params
        .equipment_roots
        .get(parent)
        .map_err(|_| "Spa part parent is not equipment root")?;
    if visual.owner != owner || root.observed_identity() != Some(&set.manifest.identity) {
        return Err("Spa part belongs to a different owner/generation".into());
    }
    let pose = params
        .part_transforms
        .get(entity)
        .map_err(|_| "Spa part pose missing")?;
    let value = params
        .materials
        .get(&material.0)
        .ok_or("Spa part material not resident")?;
    let expected_texture = if active {
        set.image("slot_emissive")
    } else {
        None
    };
    if set.mesh(&part.mesh_role) != Some(&mesh.0)
        || !pose
            .translation
            .abs_diff_eq(Vec3::from_array(part.translation_wu), 0.001)
        || !pose
            .rotation
            .abs_diff_eq(Quat::from_array(part.rotation_xyzw), 0.001)
        || !pose.scale.abs_diff_eq(Vec3::from_array(part.scale), 0.001)
        || (require_visible && !visible.get())
        || !material_matches(
            value,
            set.image("albedo"),
            set.image("slot_emissive"),
            active,
        )
        || part.material_role
            != if name == "body" {
                BuildingPartMaterial::OpaqueAlbedo
            } else {
                BuildingPartMaterial::SpaSlot
            }
    {
        return Err(format!("Spa mask page actual part differs: {name}"));
    }
    Ok(
        json!({"entity": entity.to_bits(), "parent": parent.to_bits(), "name": name, "mesh_role": part.mesh_role, "material_role": part.material_role,
        "translation_wu": pose.translation.to_array(), "rotation_xyzw": pose.rotation.to_array(),
        "scale": pose.scale.to_array(), "visible": visible.get(), "slot_active": active,
        "emissive_white": value.base.emissive == LinearRgba::WHITE,
        "emissive_black": value.base.emissive == LinearRgba::BLACK,
        "emissive_texture_matches": value.base.emissive_texture.as_ref() == expected_texture,
        "roughness": value.base.perceptual_roughness, "metallic": value.base.metallic,
        "reflectance": value.base.reflectance, "material_values": format!("{value:?}"),
        "white_opaque_lit": value.base.base_color == Color::WHITE && !value.base.unlit && value.base.alpha_mode == AlphaMode::Opaque,
        "material": format!("{:?}", material.0.id())}),
    )
}

fn material_matches(
    value: &TopDownStructuralMaterial,
    albedo: Option<&Handle<Image>>,
    emissive: Option<&Handle<Image>>,
    active: bool,
) -> bool {
    albedo.is_some()
        && emissive.is_some()
        && value.base.base_color == Color::WHITE
        && value.base.base_color_texture.as_ref() == albedo
        && !value.base.unlit
        && value.base.alpha_mode == AlphaMode::Opaque
        && value.base.perceptual_roughness == 1.0
        && value.base.metallic == 0.0
        && value.base.reflectance == 0.0
        && value.base.emissive
            == if active {
                LinearRgba::WHITE
            } else {
                LinearRgba::BLACK
            }
        && value.base.emissive_texture.as_ref() == if active { emissive } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spa_snapshot_changes_fail_without_overwriting_the_initial_evidence() {
        let initial =
            json!({"owner": 1, "root": 2, "parts": [{"entity": 3, "material_values": "original"}]});
        assert!(unchanged(None, &initial).is_ok());
        assert!(unchanged(Some(&initial), &initial).is_ok());
        for changed in [
            json!({"owner": 4}),
            json!({"root": 4}),
            json!({"parts": [{"entity": 4}]}),
            json!({"parts": [{"entity": 3, "material_values": "modified"}]}),
        ] {
            assert!(unchanged(Some(&initial), &changed).is_err());
            assert_eq!(initial["root"], 2);
            assert_eq!(initial["parts"][0]["material_values"], "original");
        }
    }

    #[test]
    fn spa_material_observation_rejects_wrong_slot_emission_and_pbr() {
        let mut images = Assets::<Image>::default();
        let albedo = images.add(Image::default());
        let emission = images.add(Image::default());
        for active in [false, true] {
            let value = TopDownStructuralMaterial {
                base: StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: Some(albedo.clone()),
                    perceptual_roughness: 1.0,
                    metallic: 0.0,
                    reflectance: 0.0,
                    emissive: if active {
                        LinearRgba::WHITE
                    } else {
                        LinearRgba::BLACK
                    },
                    emissive_texture: active.then(|| emission.clone()),
                    ..default()
                },
                ..default()
            };
            assert!(material_matches(
                &value,
                Some(&albedo),
                Some(&emission),
                active
            ));
            assert!(!material_matches(
                &value,
                Some(&albedo),
                Some(&emission),
                !active
            ));
            assert!(!material_matches(&value, None, Some(&emission), active));
            assert!(!material_matches(&value, Some(&albedo), None, active));
            let mut wrong = value.clone();
            wrong.base.unlit = true;
            assert!(!material_matches(
                &wrong,
                Some(&albedo),
                Some(&emission),
                active
            ));
            wrong = value.clone();
            wrong.base.base_color_texture = Some(emission.clone());
            assert!(!material_matches(
                &wrong,
                Some(&albedo),
                Some(&emission),
                active
            ));
            wrong = value.clone();
            wrong.base.reflectance = 0.5;
            assert!(!material_matches(
                &wrong,
                Some(&albedo),
                Some(&emission),
                active
            ));
            wrong = value.clone();
            wrong.base.perceptual_roughness = f32::NAN;
            assert!(!material_matches(
                &wrong,
                Some(&albedo),
                Some(&emission),
                active
            ));
        }
    }
}
