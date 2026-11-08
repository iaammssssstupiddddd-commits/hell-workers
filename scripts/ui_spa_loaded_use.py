"""Bounded ordinary Load continuation; an old save is input, not current evidence."""
from pathlib import Path
import re
import shutil

from scripts import ui_spa_resumed_use as resumed

art, spa, occupied, native = resumed.art, resumed.spa, resumed.occupied, resumed.native
pipeline = art.pipeline
CASE, SCOPE = art.LOADED_CASE, art.LOADED_SCOPE
CHECKPOINTS = (occupied.CHECKPOINTS[0], "soul-spa-candidate-input-loaded") + resumed.CHECKPOINTS[-3:]


def origin_contract(repo, root, candidate):
    root = pipeline.no_symlinks(root)
    native.require(root.parent == repo / "target/native-acceptance" and root.is_dir(),
                   "Spa origin requires a canonical retained native job")
    manifest = native.read_json(root / "manifest.json")
    job = native.read_json(root / "job.json")
    native.require(job.get("status") == "valid" and "child_pid" in job and job["child_pid"] is None
                   and job.get("failure") is None and isinstance(job.get("completed_at"), str)
                   and job.get("profile") == manifest.get("profile") == "ui-usability-feedback"
                   and manifest.get("evidence_kind") == "feedback"
                   and manifest.get("input_backend") in ("portal", "portal-eis"),
                   "Spa origin must be a successful ended candidate job")
    native.require(manifest.get("case") == art.CASE and manifest.get("smoke") is True
                   and manifest.get("repo") == str(repo) and len(manifest.get("sessions", [])) == 1,
                   "Spa origin is not a single occupied candidate campaign")
    source = manifest["ordinary_art"]
    for key in ("identity", "manifest_sha256", "artifacts", "parts", "relative_tiles",
                "codec", "codec_sha256", "source_root", "base_assets_sha256", "numeric_verifiers"):
        native.require(pipeline.exact_json(source.get(key), candidate.get(key)), "Spa origin candidate identity differs")
    session = manifest["sessions"][0]
    native.require(session.get("directory") == "viewport-0" and session.get("case") == art.CASE,
                   "Spa origin session differs")
    directory = root / "viewport-0"
    paths = [root / "manifest.json"] + [directory / name for name in
        ("checkpoints.json", "events.json", "input-client.json", "production-cancellation.json", "saves/manual-1.scn.ron")]
    raw = native.read_json(paths[4])
    entries = native.read_json(paths[1])["checkpoints"]
    inputs = native.read_json(paths[2])["events"]
    client = native.read_json(paths[3])
    native.require(client.get("input_backend") == manifest["input_backend"]
                   and type(client.get("portal_devices")) is int and client["portal_devices"] & 3 == 3
                   and isinstance(client.get("monitor_mapping"), list) and len(client["monitor_mapping"]) == 2
                   and spa.finite(client["monitor_mapping"][1]) and client["monitor_mapping"][1] > 0,
                   "Spa origin transport differs")
    if manifest["input_backend"] == "portal-eis":
        art.shared.check_eis_client(client, scroll=False)
    import wall_door_joint_acceptance as joint
    log = directory / "game.log"
    native.require(pipeline.digest(log.read_bytes()) == session.get("log_sha256"), "Spa origin log changed")
    joint.verify_game_log(log, "Intel")
    # Historical self-contained domain/transport validation, deliberately not
    # the generic verifier which demands today's HEAD/binary/harness.
    occupied.verify(raw, entries, client, paths[5], inputs, raw_scope=art.SCOPE)
    art.check_consumers(raw, entries, source)
    native.require(session.get("nonce") == client.get("nonce")
                   and session.get("cancellation_sha256") == pipeline.digest(paths[4].read_bytes()),
                   "Spa origin session/raw differs")
    for entry in entries:
        path = pipeline.rooted(directory, entry["screenshot"])
        native.require(path.suffix == ".png" and pipeline.digest(path.read_bytes()) == entry["sha256"]
                       and entry.get("capture_scope") == "x11-client-window"
                       and entry.get("pid") == client["pid"] and entry.get("window") == client["window"],
                       "Spa origin owned capture differs")
        paths.append(path)
    paths.extend((root / "job.json", log))
    inventory = [{"path": str(pipeline.no_symlinks(path)), "sha256": pipeline.digest(path.read_bytes()),
                  "bytes": path.stat().st_size} for path in paths]
    return {"root": str(root), "files": inventory, "save_sha256": inventory[5]["sha256"],
            "save_bytes": inventory[5]["bytes"], "saved": spa.context(entries[5]["value"]),
            "planning": {key: entries[5]["value"]["spa_planning"][key] for key in ("anchor", "footprint", "draw_pos")},
            "historical_subject": manifest["subject_commit"], "historical_source": manifest["source_fingerprint"]}


def check_origin(repo, contract):
    native.require(pipeline.exact_json(origin_contract(repo, Path(contract["resume_input"]["root"]), contract),
                                       contract["resume_input"]), "Spa immutable origin changed")


def prepare_input(repo, directory, contract):
    check_origin(repo, contract)
    target = directory / "saves/manual-1.scn.ron"
    native.require(not target.parent.exists(), "Spa imported save destination must be fresh")
    target.parent.mkdir()
    source = Path(contract["resume_input"]["files"][5]["path"])
    shutil.copyfile(source, target)
    check_save(target, contract)
    check_origin(repo, contract)


def check_save(path, contract):
    native.require(path.name == "manual-1.scn.ron" and path.parent.name == "saves"
                   and pipeline.no_symlinks(path) == path and path.is_file()
                   and path.stat().st_size == contract["resume_input"]["save_bytes"]
                   and pipeline.digest(path.read_bytes()) == contract["resume_input"]["save_sha256"],
                   "Spa imported save changed")


def check_observation(case, value):
    if case == CHECKPOINTS[1]:
        native.require(value.get("paused") is True, "Spa loaded input capture must be paused")
        occupied.runtime_reset(spa.context(value))
    else:
        resumed.check_observation(case, value)


def check_inputs(raw, entries, client, inputs):
    actions = spa.check_action_inventory(raw, entries, client, inputs)
    native.require(len(actions) == 9 and actions[0]["key"] == "F9"
                   and actions[1]["key"].startswith("menu:SelectLoadCatalogSlot ")
                   and actions[2]["key"].startswith("menu:ConfirmLoadCatalogSlot ")
                   and [a["key"] for a in actions[3:]] == [f"menu:SetTimeSpeed({speed})"
                        for speed in ("Super", "Paused", "Normal", "Paused", "Super", "Paused")],
                   "Spa load continuation input sequence differs")
    selected = re.fullmatch(r"menu:SelectLoadCatalogSlot \{ slot: Manual1, session: ([1-9][0-9]*) \}", actions[1]["key"])
    confirmed = re.fullmatch(r"menu:ConfirmLoadCatalogSlot \{ slot: Manual1, session: ([1-9][0-9]*) \}", actions[2]["key"])
    native.require(selected is not None and confirmed is not None and selected[1] == confirmed[1],
                   "Spa imported slot/dialog differs")
    for index, group in enumerate((actions[:3], actions[3:5], actions[5:7], actions[7:9])):
        lower, upper = entries[index:index + 2]
        for action in group:
            a, b = action["before"], action["after"]
            native.require(lower["last_step"] < action["first_step"] <= action["last_step"] <= upper["last_step"]
                           and lower["value"]["frame"] <= a["frame"] < b["frame"] < upper["value"]["frame"]
                           and a["world_epoch"] == lower["value"]["world_epoch"]
                           and b["world_epoch"] == (upper["value"]["world_epoch"] if index == 0 and action is group[-1]
                                                    else lower["value"]["world_epoch"]), "Spa load continuation input phase differs")
        if index == 0:
            native.require(all(a[side]["paused"] is True for a in group for side in ("before", "after")),
                           "Spa input load must remain paused")
        else:
            native.require(group[0]["before"]["paused"] is True and group[0]["after"]["paused"] is False
                           and group[1]["before"]["paused"] is False and group[1]["after"]["paused"] is True
                           and all(spa.context(a["after"])["relative_speed"] == (4, 1, 4)[index - 1] for a in group),
                           "Spa load continuation HUD speed differs")
    return actions


def verify(raw, entries, client, save_path, inputs, contract):
    native.require(contract.get("case") == CASE and contract.get("scope") == SCOPE
                   and [entry["case"] for entry in entries] == list(CHECKPOINTS), "Spa loaded case/sequence differs")
    spa.check_raw_session(raw, client, raw_scope=SCOPE)
    actions = check_inputs(raw, entries, client, inputs)
    check_save(save_path, contract)
    values = [e["value"] for e in entries]
    spa.check_completion_observation(CHECKPOINTS[0], values[0])
    saved, loaded = contract["resume_input"]["saved"], spa.context(values[1])
    for entry in entries:
        value = entry["value"]
        native.require(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and type(entry.get("last_step")) is int
                       and entry["last_step"] == sum(row["post_frame"] < value["frame"] for row in inputs)
                       and all(value["spa_planning"][key] == contract["resume_input"]["planning"][key]
                               for key in ("anchor", "footprint", "draw_pos")), "Spa imported world/target differs")
    snapshots = [spa.raw_checkpoint(raw, v) for v in values]
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
                   and values[1]["world_epoch"] == values[0]["world_epoch"] + 1, "Spa loaded order/epoch differs")
    receipts = [r for r in raw["events"] if r.get("operation") in ("DurableLoad", "DurableSave")]
    native.require(len(receipts) == 1 and receipts[0].get("operation") == "DurableLoad"
                   and not any(r.get("operation") in ("SoulSpaSlotsChange", "SoulSpaConstructionCancel") for r in raw["events"]),
                   "Spa loaded foreign/missing domain operation")
    receipt = receipts[0]
    native.require(receipt.get("producer") == "ordinary-load-commit-v1" and receipt.get("result") == "Committed"
                   and receipt.get("target") == "Manual slot 1"
                   and re.fullmatch(r"Load\(NormalCatalog \{ dialog_session: [1-9][0-9]* \}\)", receipt.get("request_source", ""))
                   and receipt.get("file_sha256") == contract["resume_input"]["save_sha256"]
                   and type(receipt.get("file_bytes")) is int and receipt["file_bytes"] == contract["resume_input"]["save_bytes"]
                   and type(receipt.get("previous_epoch")) is int and receipt["previous_epoch"] == values[0]["world_epoch"]
                   and type(receipt.get("world_epoch")) is int and receipt["world_epoch"] == values[1]["world_epoch"]
                   and type(receipt.get("sample_index")) is int
                   and spa.finite(receipt.get("real_seconds"))
                   and snapshots[0]["sample_index"] < receipt["sample_index"] <= snapshots[1]["sample_index"],
                   "Spa imported load receipt differs")
    spa.after_press(receipt, spa.pressed_for(raw, actions[2]))
    sample = raw["samples"][receipt["sample_index"]]
    native.require(actions[2]["before"]["spa_planning"]["real_seconds"] < receipt["real_seconds"]
                   <= values[1]["spa_planning"]["real_seconds"]
                   and receipt["real_seconds"] <= sample["real_seconds"] <= snapshots[1]["real_seconds"]
                   and sample.get("world_epoch") == receipt["world_epoch"] and sample.get("paused") is True
                   and sample.get("primary_window") == snapshots[1]["primary_window"]
                   and sample.get("primary_window_bits") == snapshots[1]["primary_window_bits"],
                   "Spa imported load commit time/window differs")
    session = re.search(r"session: ([1-9][0-9]*) \}", actions[2]["key"])
    native.require(session is not None and f"dialog_session: {session[1]} }}" in receipt["request_source"],
                   "Spa imported load dialog differs")
    occupied.mapped_durable(saved, loaded, receipt.get("spa_entity_remap"))
    occupied.mapped_souls(saved, loaded, receipt.get("entity_remap"))
    for sample in raw["samples"][snapshots[0]["sample_index"]:snapshots[1]["sample_index"] + 1]:
        post = sample["sample_index"] >= receipt["sample_index"]
        state = spa.context({"spa_planning": {"context": sample["spa_context"]}})
        native.require(sample.get("paused") is True
                       and sample.get("world_epoch") == values[int(post)]["world_epoch"]
                       and sample.get("primary_window") == snapshots[0]["primary_window"]
                       and sample.get("primary_window_bits") == snapshots[0]["primary_window_bits"]
                       and sample.get("virtual_seconds") == values[int(post)]["spa_planning"]["virtual_seconds"]
                       and occupied.exact(state, loaded if post else spa.context(values[0])),
                       "Spa paused import continuity differs")
    native.require(occupied.exact(receipt.get("spa_context"), loaded), "Spa load commit/reset capture differs")
    art.check_consumers(raw, entries, contract, start=1)
    resumed.verify_tail(raw, entries, client, inputs, saved, loaded_index=1, speeds=(4, 1, 4))


def exercise(driver, contract):
    driver.capture(CHECKPOINTS[0])
    native.require(all(driver.last["spa_planning"][key] == contract["resume_input"]["planning"][key]
                       for key in ("anchor", "footprint", "draw_pos")), "Spa origin differs from actual prepared target")
    spa.tap(driver, "F9")
    driver.last = driver.wait(lambda v: any(k.startswith("menu:SelectLoadCatalogSlot") for k in v["controls"]), after=driver.last["frame"])
    spa.click(driver, spa.catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    spa.click(driver, spa.catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda v: v["world_epoch"] == epoch + 1 and v["catalog"] == "Closed", after=driver.last["frame"], timeout=60)
    native.require(driver.last.get("paused") is True, "Spa ordinary load unexpectedly resumed")
    check_observation(CHECKPOINTS[1], driver.last)
    driver.capture(CHECKPOINTS[1])
    raw = native.read_json(driver.root / "production-cancellation.json")
    saved, loaded = contract["resume_input"]["saved"], spa.context(driver.last)
    lookup = resumed.resumed_workers(saved, loaded, raw["events"])
    resumed.exercise_tail(driver, {lookup[key] for key in spa.generating(saved)}, normal_measurement=True)
