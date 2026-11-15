//! Passive material facts for loaded M6 parts, sampled outside measurement.
use super::*;
use crate::assets::building_asset_set::BuildingAssetDescriptor;
use std::collections::BTreeMap;

pub(super) fn observe(
    params: &InspectParams,
    specimen: &Specimen,
    set: &BuildingAssetDescriptor,
    owner: Entity,
    entity: Entity,
) -> Result<Value, String> {
    let name = params
        .part_names
        .get(entity)
        .map_err(|_| "M6 resident part name missing")?
        .as_str();
    let authored = set
        .manifest
        .parts
        .iter()
        .find(|part| part.name == name)
        .ok_or("M6 resident name is not in the loaded descriptor")?;
    let (_, _, parent, mesh, material, _) = params
        .visuals
        .get(entity)
        .map_err(|_| "M6 resident visual missing")?;
    let parent = parent.ok_or("M6 resident parent missing")?.parent();
    let (root_entity, visual, root) = params
        .equipment_roots
        .get(parent)
        .map_err(|_| "M6 resident parent is not an equipment root")?;
    let root_pose = params
        .part_transforms
        .get(root_entity)
        .map_err(|_| "M6 resident root pose missing")?;
    if root_entity != parent
        || visual.owner != owner
        || root.observed_identity() != Some(&set.manifest.identity)
        || set.mesh(&authored.mesh_role) != Some(&mesh.0)
        || !root_pose
            .translation
            .abs_diff_eq(Vec3::new(specimen.center.x, 0.0, -specimen.center.y), 0.001)
        || !root_pose.rotation.abs_diff_eq(Quat::IDENTITY, 0.001)
        || !root_pose.scale.abs_diff_eq(Vec3::ONE, 0.001)
    {
        return Err("M6 resident owner/root/identity/mesh/pose differs".into());
    }
    let value = params
        .materials
        .get(&material.0)
        .ok_or("M6 resident material missing")?;
    let meshes = set
        .manifest
        .identity
        .kind
        .mesh_roles()
        .iter()
        .map(|role| {
            set.mesh(role)
                .map(|handle| (*role, format!("{:?}", handle.id())))
                .ok_or("M6 resident descriptor mesh missing")
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let images = set
        .manifest
        .identity
        .kind
        .image_roles()
        .iter()
        .map(|role| {
            set.image(role)
                .map(|handle| (*role, format!("{:?}", handle.id())))
                .ok_or("M6 resident descriptor image missing")
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let spa = if set.manifest.identity.kind == BuildingAssetKind::SoulSpa {
        // Reuse the actual parent, pose and worker-mask/emission guard. The
        // texture handle is retained as a fact, not only a "matches" boolean.
        let observed = spa_masks::observe_resident(params, specimen, set, owner, entity)?;
        json!({
            "slot_active": observed["slot_active"],
            "emissive_white": observed["emissive_white"],
            "emissive_black": observed["emissive_black"],
            "texture": value.base.emissive_texture.as_ref().map(|h| format!("{:?}", h.id())),
        })
    } else {
        Value::Null
    };
    Ok(json!({
        "entity": entity.to_bits(), "parent": parent.to_bits(),
        "root": root_entity.to_bits(), "owner": visual.owner.to_bits(),
        "root_identity": root.observed_identity(), "name": name,
        "root_pose": {"translation": root_pose.translation.to_array(),
            "rotation": root_pose.rotation.to_array(), "scale": root_pose.scale.to_array()},
        "mesh": format!("{:?}", mesh.0.id()), "material": format!("{:?}", material.0.id()),
        "texture": value.base.base_color_texture.as_ref().map(|h| format!("{:?}", h.id())),
        "meshes": meshes, "images": images,
        "body_paint": crate::systems::visual::building_presentation::body_paint_snapshot(value),
        "spa": spa,
    }))
}
