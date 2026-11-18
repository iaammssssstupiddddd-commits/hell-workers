//! Opt-in profiling receipts from the real save/normal-load commit branches.
//! Never reflected, persisted, used for gameplay, or a source of acceptance authority.
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use hw_core::{WorldEpoch, soul::DamnedSoul};
use hw_energy::{PowerGrid, SoulSpaSite, SoulSpaTile};
use hw_infra::lighting::digest_hex;
use hw_jobs::RestArea;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::format::{SaveHeader, decode_save_file, encode_save_header_text};
use super::{
    DispatchedSaveLoadRequest, LoadRequestOrigin, SaveLoadOperation, SaveLoadOutcomeSource,
    SaveLoadRequest,
};

const RECEIPT_LIMIT: usize = 8;
const MAPPING_LIMIT: usize = 640;
const INPUT_LIMIT: usize = 64 * 1024 * 1024;

mod m2;

#[derive(Resource, Default)]
pub(crate) struct DurableReceipts {
    pub rows: Vec<Value>,
    pub failure: Option<String>,
    pending: Option<PendingLoad>,
}

struct PendingLoad {
    request: SaveLoadRequest,
    epoch: u64,
    file_hash: String,
    file_bytes: usize,
    mapping: Option<Vec<Value>>,
    spa_mapping: Option<Vec<Value>>,
    m2_observed: bool,
    m2_mapping: Option<Vec<Value>>,
}

fn request(world: &World, operation: SaveLoadOperation) -> Result<SaveLoadRequest, String> {
    let request = world
        .get_resource::<DispatchedSaveLoadRequest>()
        .and_then(|request| request.0.clone())
        .ok_or("durable observation missing dispatched request")?;
    if request.operation() != operation
        || matches!(
            request,
            SaveLoadRequest::Load {
                origin: LoadRequestOrigin::RecoveryCatalog { .. },
                ..
            }
        )
    {
        return Err("durable observation requires the exact ordinary request".into());
    }
    Ok(request)
}

fn poison(world: &mut World, error: String) {
    if let Some(mut receipts) = world.get_resource_mut::<DurableReceipts>() {
        receipts.failure.get_or_insert(error);
        receipts.pending = None;
    }
}

fn push(world: &mut World, mut row: Value, request: &SaveLoadRequest) -> Result<(), String> {
    let epoch = world
        .get_resource::<WorldEpoch>()
        .ok_or("durable observation missing epoch")?
        .get();
    let now = world
        .get_resource::<Time<Real>>()
        .ok_or("durable observation missing real time")?
        .elapsed_secs_f64();
    row["world_epoch"] = json!(epoch);
    row["real_seconds"] = json!(now);
    row["target"] = json!(request.target_label());
    row["request_source"] = json!(format!(
        "{:?}",
        SaveLoadOutcomeSource::from_request(request)
    ));
    row["rest_area_context"] =
        crate::assets::building_asset_set::snapshot_rest_observations(world)?;
    row["spa_context"] = crate::assets::building_asset_set::snapshot_spa_observations(world)?;
    if crate::assets::building_asset_set::m2_persistence_enabled(world) {
        row["m2_context"] = crate::assets::building_asset_set::snapshot_m2_observations(world)?;
    }
    let mut receipts = world.resource_mut::<DurableReceipts>();
    if receipts.rows.len() >= RECEIPT_LIMIT {
        return Err("durable observation receipt cap exceeded".into());
    }
    receipts.rows.push(row);
    Ok(())
}

pub(super) fn saved(world: &mut World, header: SaveHeader, body: &str) {
    if !world.contains_resource::<DurableReceipts>() {
        return;
    }
    let result = (|| {
        if body.len() > INPUT_LIMIT {
            return Err("durable observation body cap exceeded".into());
        }
        let request = request(world, SaveLoadOperation::Save)?;
        let header = encode_save_header_text(header);
        if header.len() + body.len() > INPUT_LIMIT {
            return Err("durable observation container cap exceeded".into());
        }
        let mut digest = Sha256::new();
        digest.update(header.as_bytes());
        digest.update(body.as_bytes());
        push(
            world,
            json!({"producer": "ordinary-save-commit-v1", "operation": "DurableSave",
        "result": "Committed", "file_sha256": digest_hex(&digest.finalize().into()),
            "file_bytes": header.len() + body.len()}),
            &request,
        )
    })();
    if let Err(error) = result {
        poison(world, error);
    }
}

pub(super) fn begin_load(world: &mut World, contents: &str) {
    if !world.contains_resource::<DurableReceipts>() {
        return;
    }
    let result = (|| {
        if contents.len() > INPUT_LIMIT || world.resource::<DurableReceipts>().pending.is_some() {
            return Err("durable observation pending/input cap exceeded".into());
        }
        decode_save_file(contents).map_err(|error| error.to_string())?;
        let pending = PendingLoad {
            request: request(world, SaveLoadOperation::Load)?,
            epoch: world
                .get_resource::<WorldEpoch>()
                .ok_or("durable observation missing epoch")?
                .get(),
            file_hash: digest_hex(&Sha256::digest(contents.as_bytes()).into()),
            file_bytes: contents.len(),
            mapping: None,
            spa_mapping: None,
            m2_observed: crate::assets::building_asset_set::m2_persistence_enabled(world),
            m2_mapping: None,
        };
        world.resource_mut::<DurableReceipts>().pending = Some(pending);
        Ok(())
    })();
    if let Err(error) = result {
        poison(world, error);
    }
}

pub(super) fn mapped(world: &mut World, mapping: &EntityHashMap<Entity>) {
    if !world.contains_resource::<DurableReceipts>() {
        return;
    }
    let result = (|| {
        if world.resource::<DurableReceipts>().pending.is_none() || mapping.len() > 65536 {
            return Err("durable observation missing pending load or mapping cap exceeded".into());
        }
        let mut rows = Vec::new();
        let mut targets = std::collections::HashSet::new();
        for (&saved, &loaded) in mapping {
            let kind = if world.get::<DamnedSoul>(loaded).is_some() {
                "Soul"
            } else if world.get::<RestArea>(loaded).is_some() {
                "RestArea"
            } else {
                continue;
            };
            if !targets.insert(loaded) || rows.len() >= MAPPING_LIMIT {
                return Err(
                    "durable observation duplicate target or selected mapping cap exceeded".into(),
                );
            }
            rows.push(json!({"saved_entity": format!("{saved:?}"), "saved_bits": saved.to_bits(),
                "loaded_entity": format!("{loaded:?}"), "loaded_bits": loaded.to_bits(), "kind": kind}));
        }
        let expected = world
            .query_filtered::<Entity, Or<(With<DamnedSoul>, With<RestArea>)>>()
            .iter(world)
            .count();
        if rows.len() != expected {
            return Err("durable observation incomplete Soul/RestArea map".into());
        }
        rows.sort_by_key(|row| row["saved_bits"].as_u64());
        // Keep the existing Soul/RestArea map unchanged for its consumers.
        // Spa uses durable parent_site, not the non-persisted display hierarchy.
        let mut spa_rows = Vec::new();
        let mut spa_targets = std::collections::HashSet::new();
        for (&saved, &loaded) in mapping {
            let kind = match (
                world.get::<SoulSpaSite>(loaded),
                world.get::<SoulSpaTile>(loaded),
                world.get::<PowerGrid>(loaded),
            ) {
                (Some(_), None, None) => "SoulSpa",
                (None, Some(_), None) => "SoulSpaTile",
                (None, None, Some(_)) => "PowerGrid",
                (None, None, None) => continue,
                _ => return Err("durable Spa mapping has ambiguous components".into()),
            };
            if !spa_targets.insert(loaded) || spa_rows.len() >= 3072 {
                return Err("durable Spa mapping duplicate target/cap exceeded".into());
            }
            spa_rows.push(json!({"saved_entity": format!("{saved:?}"), "saved_bits": saved.to_bits(),
                "loaded_entity": format!("{loaded:?}"), "loaded_bits": loaded.to_bits(), "kind": kind}));
        }
        let spa_expected = world
            .query_filtered::<Entity, Or<(With<SoulSpaSite>, With<SoulSpaTile>, With<PowerGrid>)>>()
            .iter(world)
            .count();
        if spa_rows.len() != spa_expected {
            return Err("durable Spa mapping is incomplete".into());
        }
        spa_rows.sort_by_key(|row| row["saved_bits"].as_u64());
        let m2_observed = crate::assets::building_asset_set::m2_persistence_enabled(world);
        if world
            .resource::<DurableReceipts>()
            .pending
            .as_ref()
            .map(|p| p.m2_observed)
            != Some(m2_observed)
        {
            return Err("durable M2 observation scope changed".into());
        }
        let m2_mapping = m2_observed
            .then(|| m2::mapping(world, mapping))
            .transpose()?;
        let mut receipts = world.resource_mut::<DurableReceipts>();
        let pending = receipts
            .pending
            .as_mut()
            .ok_or("durable observation pending disappeared")?;
        if pending.mapping.is_some() {
            return Err("durable observation duplicate map".into());
        }
        pending.mapping = Some(rows);
        pending.spa_mapping = Some(spa_rows);
        pending.m2_mapping = m2_mapping;
        Ok(())
    })();
    if let Err(error) = result {
        poison(world, error);
    }
}

pub(super) fn finish_load(world: &mut World, success: bool) {
    let Some(mut receipts) = world.get_resource_mut::<DurableReceipts>() else {
        return;
    };
    let pending = receipts.pending.take();
    if !success {
        return;
    }
    let result = (|| {
        let pending =
            pending.ok_or("durable observation successful load has no pending bytes/map")?;
        let current = request(world, SaveLoadOperation::Load)?;
        if current != pending.request
            || world.get_resource::<WorldEpoch>().map(|epoch| epoch.get())
                != Some(pending.epoch.wrapping_add(1))
        {
            return Err("durable observation request/epoch changed".into());
        }
        let mapping = pending
            .mapping
            .ok_or("durable observation successful load has no applied map")?;
        let spa_mapping = pending
            .spa_mapping
            .ok_or("durable observation successful load has no Spa map")?;
        if pending.m2_observed != crate::assets::building_asset_set::m2_persistence_enabled(world) {
            return Err("durable M2 observation scope changed".into());
        }
        let mut row = json!({"producer": "ordinary-load-commit-v1", "operation": "DurableLoad",
            "result": "Committed", "file_sha256": pending.file_hash, "file_bytes": pending.file_bytes,
            "previous_epoch": pending.epoch, "entity_remap": mapping, "spa_entity_remap": spa_mapping});
        if pending.m2_observed {
            row["m2_entity_remap"] = json!(pending.m2_mapping.ok_or("durable M2 load has no map")?);
        }
        push(world, row, &current)
    })();
    if let Err(error) = result {
        poison(world, error);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{SaveRequestOrigin, catalog::SaveExpectedTarget, format::encode_save_file};
    use super::*;
    use hw_core::SaveSlotId;

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<DurableReceipts>();
        world.init_resource::<WorldEpoch>();
        world.init_resource::<Time<Real>>();
        world.insert_resource(DispatchedSaveLoadRequest(Some(SaveLoadRequest::Load {
            origin: LoadRequestOrigin::NormalCatalog { dialog_session: 4 },
            slot: SaveSlotId::Manual1,
            expected_revision: None,
        })));
        world
    }

    fn bytes() -> String {
        encode_save_file(SaveHeader::current(20260914), "(resources:[],entities:{})")
    }

    #[test]
    fn ordinary_world_without_observer_has_no_diagnostic_resources_or_mutation() {
        let mut world = World::new();
        let before = world.entities().len();
        saved(&mut world, SaveHeader::current(0), "test body");
        begin_load(&mut world, "not a valid container");
        mapped(&mut world, &default());
        finish_load(&mut world, true);
        assert!(!world.contains_resource::<DurableReceipts>());
        assert_eq!(world.entities().len(), before);
    }

    #[test]
    fn explicit_m2_session_publishes_only_actual_commit_snapshot_and_applied_map() {
        let mut world = world();
        world.insert_resource(crate::assets::building_asset_set::m2_persistence_test_session());
        let old = world.spawn_empty().id();
        begin_load(&mut world, &bytes());
        world.despawn(old);
        let tank = world
            .spawn((
                hw_jobs::Building {
                    kind: hw_jobs::BuildingType::Tank,
                    is_provisional: false,
                },
                Transform::default(),
            ))
            .id();
        let before = world.entities().len();
        world.resource_mut::<WorldEpoch>().advance();
        mapped(&mut world, &EntityHashMap::from_iter([(old, tank)]));
        assert!(world.resource::<DurableReceipts>().rows.is_empty());
        finish_load(&mut world, true);
        let receipts = world.resource::<DurableReceipts>();
        assert!(receipts.failure.is_none(), "{:?}", receipts.failure);
        let row = &receipts.rows[0];
        assert_eq!(row["m2_context"]["tanks"][0]["entity_bits"], tank.to_bits());
        assert_eq!(
            row["m2_entity_remap"],
            json!([{
                "saved_entity": format!("{old:?}"), "saved_bits": old.to_bits(),
                "loaded_entity": format!("{tank:?}"), "loaded_bits": tank.to_bits(), "kind": "Tank"
            }])
        );
        assert_eq!(world.entities().len(), before);
    }

    #[test]
    fn m2_scope_drift_incomplete_map_and_failed_load_cannot_publish_success() {
        for case in 0..4 {
            let mut world = world();
            if case != 0 {
                world.insert_resource(
                    crate::assets::building_asset_set::m2_persistence_test_session(),
                );
            }
            begin_load(&mut world, &bytes());
            if case == 0 {
                world.insert_resource(
                    crate::assets::building_asset_set::m2_persistence_test_session(),
                );
            }
            let tank = world
                .spawn(hw_jobs::Building {
                    kind: hw_jobs::BuildingType::Tank,
                    is_provisional: false,
                })
                .id();
            world.resource_mut::<WorldEpoch>().advance();
            let mapping = if case == 1 {
                EntityHashMap::default()
            } else {
                EntityHashMap::from_iter([(tank, tank)])
            };
            mapped(&mut world, &mapping);
            if case == 2 {
                world
                    .resource_mut::<DurableReceipts>()
                    .pending
                    .as_mut()
                    .unwrap()
                    .m2_mapping = None;
            }
            finish_load(&mut world, case != 3);
            let receipts = world.resource::<DurableReceipts>();
            assert!(receipts.rows.is_empty());
            assert_eq!(receipts.failure.is_some(), case != 3);
            assert_eq!(
                world.get::<hw_jobs::Building>(tank).unwrap().kind,
                hw_jobs::BuildingType::Tank
            );
        }
    }

    #[test]
    fn successful_save_receipt_binds_the_exact_streamed_header_body_and_request() {
        let mut world = world();
        world.resource_mut::<DispatchedSaveLoadRequest>().0 = Some(SaveLoadRequest::Save {
            origin: SaveRequestOrigin::ManualCatalog { dialog_session: 3 },
            slot: SaveSlotId::Manual1,
            expected_target: SaveExpectedTarget::Absent,
        });
        let body = "(resources:[],entities:{})";
        saved(&mut world, SaveHeader::current(20260914), body);
        let receipts = world.resource::<DurableReceipts>();
        assert!(receipts.failure.is_none());
        assert_eq!(receipts.rows.len(), 1);
        let row = &receipts.rows[0];
        assert_eq!(
            row["file_sha256"],
            digest_hex(&Sha256::digest(bytes().as_bytes()).into())
        );
        assert_eq!(row["file_bytes"], bytes().len());
        assert_eq!(row["target"], SaveSlotId::Manual1.player_label());
        assert_eq!(row["operation"], "DurableSave");
        assert!(row.get("m2_context").is_none());
        assert!(row.get("m2_entity_remap").is_none());
        assert_eq!(
            row["request_source"],
            "Manual(ManualCatalog { dialog_session: 3 })"
        );
        assert_eq!(world.resource::<WorldEpoch>().get(), 0);
    }

    #[test]
    fn normal_load_records_only_actual_selected_map_after_success_and_epoch_change() {
        let mut world = world();
        begin_load(&mut world, &bytes());
        let saved_owner = world.spawn_empty().id();
        world.despawn(saved_owner);
        let owner = world.spawn(RestArea { capacity: 4 }).id();
        let soul = world.spawn(DamnedSoul::default()).id();
        let mapping = EntityHashMap::from_iter([(saved_owner, owner), (soul, soul)]);
        world.resource_mut::<WorldEpoch>().advance();
        mapped(&mut world, &mapping);
        assert!(world.resource::<DurableReceipts>().rows.is_empty());
        finish_load(&mut world, true);
        let receipts = world.resource::<DurableReceipts>();
        assert!(receipts.failure.is_none(), "{:?}", receipts.failure);
        let row = &receipts.rows[0];
        assert_eq!(row["previous_epoch"], 0);
        assert!(row.get("m2_context").is_none());
        assert!(row.get("m2_entity_remap").is_none());
        assert_eq!(row["world_epoch"], 1);
        assert_eq!(
            row["file_sha256"],
            digest_hex(&Sha256::digest(bytes().as_bytes()).into())
        );
        assert_eq!(row["entity_remap"].as_array().unwrap().len(), 2);
        assert!(row["entity_remap"].as_array().unwrap().iter().any(|item| {
            item["saved_bits"] == saved_owner.to_bits()
                && item["loaded_bits"] == owner.to_bits()
                && item["kind"] == "RestArea"
        }));
        assert_eq!(world.get::<RestArea>(owner).unwrap().capacity, 4);
    }

    #[test]
    fn spa_receipt_keeps_actual_site_tile_grid_remap_separate_from_rest_map() {
        let mut world = world();
        begin_load(&mut world, &bytes());
        let grid = world.spawn(PowerGrid::default()).id();
        let site = world
            .spawn((SoulSpaSite::default(), hw_energy::GeneratesFor(grid)))
            .id();
        let tiles: Vec<_> = (0..4)
            .map(|i| {
                world
                    .spawn(SoulSpaTile {
                        parent_site: site,
                        grid_pos: (i, 2),
                    })
                    .id()
            })
            .collect();
        let entities = [vec![site, grid], tiles].concat();
        let mapping = EntityHashMap::from_iter(entities.into_iter().map(|entity| (entity, entity)));
        world.resource_mut::<WorldEpoch>().advance();
        mapped(&mut world, &mapping);
        finish_load(&mut world, true);
        let receipts = world.resource::<DurableReceipts>();
        assert!(receipts.failure.is_none(), "{:?}", receipts.failure);
        let row = &receipts.rows[0];
        assert_eq!(row["entity_remap"], json!([]));
        assert_eq!(row["spa_entity_remap"].as_array().unwrap().len(), 6);
        assert_eq!(row["spa_context"]["sites"][0]["grid"], format!("{grid:?}"));
        assert_eq!(row["spa_context"]["tiles"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn incomplete_duplicate_or_ambiguous_spa_mapping_cannot_publish_a_receipt() {
        for case in 0..3 {
            let mut world = world();
            begin_load(&mut world, &bytes());
            let site = world.spawn(SoulSpaSite::default()).id();
            let tile = world
                .spawn(SoulSpaTile {
                    parent_site: site,
                    grid_pos: (1, 2),
                })
                .id();
            let mut mapping = EntityHashMap::from_iter([(site, site), (tile, tile)]);
            match case {
                0 => {
                    mapping.remove(&tile);
                }
                1 => {
                    let other = world.spawn_empty().id();
                    mapping.insert(other, tile);
                }
                _ => {
                    world.entity_mut(site).insert(PowerGrid::default());
                }
            }
            world.resource_mut::<WorldEpoch>().advance();
            mapped(&mut world, &mapping);
            finish_load(&mut world, true);
            assert!(
                world.resource::<DurableReceipts>().failure.is_some(),
                "case {case}"
            );
            assert!(world.resource::<DurableReceipts>().rows.is_empty());
        }
    }

    #[test]
    fn failed_load_discards_pending_and_cannot_reuse_a_previous_map() {
        let mut world = world();
        begin_load(&mut world, &bytes());
        mapped(&mut world, &default());
        finish_load(&mut world, false);
        assert!(world.resource::<DurableReceipts>().pending.is_none());
        assert!(world.resource::<DurableReceipts>().rows.is_empty());
        begin_load(&mut world, &bytes());
        world.resource_mut::<WorldEpoch>().advance();
        finish_load(&mut world, true);
        assert!(world.resource::<DurableReceipts>().failure.is_some());
        assert!(world.resource::<DurableReceipts>().rows.is_empty());
    }

    #[test]
    fn invalid_container_duplicate_pending_and_input_cap_are_not_success_receipts() {
        for contents in [
            "HELL_WORKERS_SAVE\nmalformed".to_owned(),
            "x".repeat(INPUT_LIMIT + 1),
        ] {
            let mut world = world();
            begin_load(&mut world, &contents);
            assert!(world.resource::<DurableReceipts>().failure.is_some());
            assert!(world.resource::<DurableReceipts>().pending.is_none());
            assert!(world.resource::<DurableReceipts>().rows.is_empty());
        }
        let mut world = world();
        begin_load(&mut world, &bytes());
        begin_load(&mut world, &bytes());
        assert!(world.resource::<DurableReceipts>().failure.is_some());
        assert!(world.resource::<DurableReceipts>().pending.is_none());
    }

    #[test]
    fn foreign_request_epoch_missing_duplicate_and_capped_observations_fail_closed() {
        for case in 0..7 {
            let mut world = world();
            begin_load(&mut world, &bytes());
            world.resource_mut::<WorldEpoch>().advance();
            match case {
                0 => world.resource_mut::<DispatchedSaveLoadRequest>().0 = None,
                1 => world.resource_mut::<WorldEpoch>().advance(),
                2 => {
                    world.spawn(RestArea { capacity: 4 });
                }
                3 => {
                    let target = world.spawn(RestArea { capacity: 4 }).id();
                    let other = world.spawn_empty().id();
                    mapped(
                        &mut world,
                        &EntityHashMap::from_iter([(target, target), (other, target)]),
                    );
                }
                4 => {
                    mapped(&mut world, &default());
                }
                5 => {
                    world.resource_mut::<DurableReceipts>().rows = vec![Value::Null; RECEIPT_LIMIT]
                }
                _ => {
                    world.resource_mut::<DispatchedSaveLoadRequest>().0 =
                        Some(SaveLoadRequest::Load {
                            origin: LoadRequestOrigin::RecoveryCatalog { dialog_session: 4 },
                            slot: SaveSlotId::Manual1,
                            expected_revision: None,
                        })
                }
            }
            mapped(&mut world, &default());
            finish_load(&mut world, true);
            assert!(
                world.resource::<DurableReceipts>().failure.is_some(),
                "case {case}"
            );
            assert!(
                world
                    .resource::<DurableReceipts>()
                    .rows
                    .iter()
                    .all(Value::is_null)
            );
        }
    }
}
