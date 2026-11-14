"""Synthetic loaded continuation and refusal tests; no native/adoption evidence."""
import copy
import json
import math
from pathlib import Path
import tempfile
import unittest

from scripts import ui_spa_loaded_use as loaded
from scripts.tests import test_ui_spa_resumed_use as fixtures


def campaign(save):
    raw, entries, client, inputs, contract = fixtures.campaign(save)
    saved = copy.deepcopy(loaded.spa.context(entries[5]["value"]))
    empty = copy.deepcopy(entries[0])
    empty.update(last_step=0)
    empty["value"].update(frame=6400)
    empty["value"]["spa_planning"].update(real_seconds=64., virtual_seconds=8.)
    initial = copy.deepcopy(loaded.spa.context(empty["value"]))
    contract.update(case=loaded.CASE, scope=loaded.SCOPE,
                    resume_input={"saved": saved, "save_sha256": loaded.pipeline.digest(save.read_bytes()),
                                  "save_bytes": save.stat().st_size,
                                  "planning": {key: empty["value"]["spa_planning"][key] for key in ("anchor", "footprint", "draw_pos")}})
    entries = [empty] + entries[6:]
    entries[1]["case"] = loaded.CHECKPOINTS[1]
    for entry in entries[1:]:
        entry["last_step"] -= 24
    inputs = inputs[24:]
    for index, event in enumerate(inputs):
        event["step"] = str(index + 1)
        action = event.get("spa_action")
        if action:
            action["first_step"] -= 24
            action["last_step"] -= 24
            for side in ("before", "after"):
                if action[side]["world_epoch"] == 0:
                    action[side]["spa_planning"]["context"] = copy.deepcopy(initial)
            if action["before"]["spa_planning"]["real_seconds"] == 80.:
                action["key"] = "menu:SetTimeSpeed(Normal)"
                action["after"]["spa_planning"]["context"]["relative_speed"] = 1.
                event["observed_hover"] = {action["key"]: "Pressed"}
            if action["before"]["spa_planning"]["real_seconds"] == 82.:
                for side in ("before", "after"):
                    action[side]["spa_planning"]["context"]["relative_speed"] = 1.
    raw["scope"] = loaded.SCOPE
    raw["samples"] = [s for s in raw["samples"] if s["real_seconds"] >= 64.]
    raw["events"] = [e for e in raw["events"] if e["real_seconds"] >= 65.]
    for sample in raw["samples"]:
        if sample["world_epoch"] == 0:
            sample["spa_context"] = copy.deepcopy(initial)
        if 80.1 <= sample["real_seconds"] < 86.:
            sample["spa_context"]["relative_speed"] = 1.
    entries[-2]["value"]["spa_planning"]["context"]["relative_speed"] = 1.
    # Give Normal's four virtual seconds at least four real seconds. Do not
    # inherit the old Super fixture's accelerated measurement clock.
    def shift(value):
        if value["spa_planning"]["real_seconds"] >= 82:
            value["frame"] += 300
            value["spa_planning"]["real_seconds"] += 3
    for event in inputs:
        for field in ("pre_frame", "post_frame"):
            if event[field] >= 8200:
                event[field] += 300
        if "spa_action" in event:
            for side in ("before", "after"):
                shift(event["spa_action"][side])
    for entry in entries:
        shift(entry["value"])
    for row in raw["samples"] + raw["events"]:
        if row["real_seconds"] >= 82:
            row["real_seconds"] += 3
    for index, sample in enumerate(raw["samples"]):
        sample["sample_index"] = index
    for index, event in enumerate(raw["events"]):
        event.update(event_index=index, sample_index=next(i for i, s in enumerate(raw["samples"])
                                                        if s["real_seconds"] >= event["real_seconds"]))
    return raw, entries, client, inputs, contract


class LoadedSpaTests(unittest.TestCase):
    def test_load_action_waits_for_world_commit_without_repeating_input(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, _, _, inputs, _ = campaign(save)
            source = next(row["spa_action"] for row in inputs
                          if row.get("spa_action", {}).get("key", "").startswith("menu:ConfirmLoadCatalogSlot "))
            key = source["key"]

            class Driver:
                def __init__(self, changed=None):
                    self.root = Path(directory)
                    self.events = []
                    self.last = copy.deepcopy(source["before"])
                    self.last.update(controls={key: {"interaction": "None"}}, catalog="LoadConfirmation",
                                     mouse_left_pressed=False, world_input_captured=False,
                                     pointer_over_ui=True, viewport=[1920, 1080], menu_state="Closed")
                    self.changed = changed
                    self.waits = []

                def point(self, target):
                    self.asserted_target = target
                    return [10., 10.]

                def send(self, **event):
                    self.events.append(event)
                    self.last["frame"] += 1
                    self.last["spa_planning"]["real_seconds"] += .1
                    self.last["controls"][key]["interaction"] = "Pressed" if event.get("pressed") else "Hovered"

                def wait(self, predicate, *, after, timeout):
                    self.waits.append((after, timeout))
                    # Release was acknowledged in the old world. The catalog
                    # stays open for a further observation before load commits.
                    for epoch, catalog in ((0, "LoadConfirmation"), (1, "Closed")):
                        value = copy.deepcopy(self.last)
                        value.update(frame=after + epoch + 1, world_epoch=epoch, catalog=catalog)
                        value["spa_planning"]["real_seconds"] += epoch + 1
                        if self.changed:
                            self.changed(value)
                        if predicate(value):
                            return value
                    raise loaded.native.AcceptanceError("synthetic load timeout; never resend")

            driver = Driver()
            loaded.confirm_load(driver)
            self.assertEqual(driver.asserted_target, key)
            self.assertEqual(driver.events[:1], [{"point": [10., 10.]}])
            self.assertEqual([{k: v for k, v in row.items() if k != "spa_action"}
                              for row in driver.events[1:]],
                             [{"button": 1, "pressed": True}, {"button": 1, "pressed": False}])
            action = driver.events[1]["spa_action"]
            self.assertEqual((action["first_step"], action["last_step"]), (2, 3))
            self.assertEqual((action["before"]["world_epoch"], action["after"]["world_epoch"]), (0, 1))
            self.assertEqual(driver.waits, [(source["before"]["frame"] + 3, 60)])

            for field, value in (("world_epoch", 0), ("world_epoch", 2), ("nonce", "foreign"),
                                 ("pid", -1), ("primary_window", -1), ("catalog", "LoadConfirmation"),
                                 ("paused", False)):
                with self.subTest(field=field, value=value):
                    driver = Driver(lambda observed: observed.update({field: value}))
                    with self.assertRaises(loaded.native.AcceptanceError):
                        loaded.confirm_load(driver)
                    self.assertEqual(len(driver.events), 3)
                    self.assertFalse(any("spa_action" in row for row in driver.events))

    def test_loaded_continuation_and_refusals(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs, contract = campaign(save)
            loaded.verify(raw, entries, client, save, inputs, contract)
            for change in ("save", "receipt", "remap", "reset", "epoch", "target", "speed", "input", "phase", "raw-speed", "dream", "departure", "future-commit", "late-index", "slot", "dialog", "early-action-close"):
                r, e, c, i, k = copy.deepcopy((raw, entries, client, inputs, contract))
                receipt = next(v for v in r["events"] if v.get("operation") == "DurableLoad")
                if change == "save":
                    k["resume_input"]["save_sha256"] = "0" * 64
                elif change == "receipt":
                    receipt["operation"] = "DurableSave"
                elif change == "remap":
                    receipt["entity_remap"][0]["loaded_entity"] = receipt["entity_remap"][0]["saved_entity"]
                elif change == "reset":
                    loaded.spa.context(e[1]["value"])["souls"][0]["task"] = "GeneratePower(...)"
                elif change == "epoch":
                    e[1]["value"]["world_epoch"] = 0
                elif change == "target":
                    k["resume_input"]["planning"]["anchor"][0] += 1
                elif change == "speed":
                    i[10]["spa_action"]["key"] = "menu:SetTimeSpeed(Super)"
                elif change == "input":
                    i[0]["spa_action"]["key"] = "F5"
                elif change == "phase":
                    e[2]["last_step"] = e[3]["last_step"]
                elif change == "raw-speed":
                    next(s for s in r["samples"] if math.isclose(s["real_seconds"], 80.2))["spa_context"]["relative_speed"] = 4.
                elif change == "dream":
                    loaded.spa.context(e[3]["value"])["souls"][0]["dream"] += 1
                elif change == "departure":
                    next(s for s in r["samples"] if s["real_seconds"] == 105.)["spa_context"]["souls"][0]["dream"] = 30.
                elif change == "future-commit":
                    receipt["real_seconds"] = e[1]["value"]["spa_planning"]["real_seconds"] + 1
                elif change == "late-index":
                    receipt["sample_index"] = len(r["samples"]) - 1
                elif change == "early-action-close":
                    next(row["spa_action"] for row in i
                         if row.get("spa_action", {}).get("key", "").startswith("menu:ConfirmLoadCatalogSlot "))["after"]["world_epoch"] = 0
                elif change in ("slot", "dialog"):
                    action = i[2]["spa_action"]
                    old = action["key"]
                    action["key"] = old.replace("Manual1", "Manual2") if change == "slot" else old.replace("session: 2", "session: 3")
                    for row in i[2:4]:
                        if old in row.get("observed_hover", {}):
                            row["observed_hover"][action["key"]] = row["observed_hover"].pop(old)
                with self.subTest(change=change), self.assertRaises(RuntimeError):
                    loaded.verify(r, e, c, save, i, k)

    def test_origin_and_copy_reject_mutation_symlink_foreign_candidate_and_existing_save(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            root = repo / "target/native-acceptance/occupied-origin"
            session = root / "viewport-0"
            save = session / "saves/manual-1.scn.ron"
            save.parent.mkdir(parents=True)
            raw, entries, client, inputs = fixtures.occupied.campaign_fixture(save)
            client.update(input_backend="portal-eis", portal_devices=3, monitor_mapping=[1, 2.],
                          input_transport="portal-eis-absolute-v1", eis_scroll=False,
                          eis_region={"scale": 2., "root_size": [2880, 1800], "rect": [0, 0, 1440, 900]})
            adopted = fixtures.candidates.contract()
            raw["scope"] = loaded.art.SCOPE
            for index, entry in enumerate(entries):
                if index >= 2:
                    fixtures.candidates.attach(loaded.spa.raw_checkpoint(raw, entry["value"]), entry["value"], adopted)
                image = f"capture-{index}.png"
                (session / image).write_bytes(b"synthetic-not-native-png")
                entry.update(screenshot=image, sha256=loaded.pipeline.digest((session / image).read_bytes()),
                             capture_scope="x11-client-window", pid=client["pid"], window=client["window"])
            for name, data in (("checkpoints.json", {"checkpoints": entries}), ("events.json", {"events": inputs}),
                               ("input-client.json", client), ("production-cancellation.json", raw)):
                (session / name).write_text(json.dumps(data))
            log = session / "game.log"
            log.write_text('AdapterInfo { name: "Intel synthetic test", backend: Vulkan }')
            job = {"profile": "ui-usability-feedback", "status": "valid", "child_pid": None, "completed_at": "synthetic-end"}
            (root / "job.json").write_text(json.dumps(job))
            manifest = {"case": loaded.art.CASE, "repo": str(repo), "smoke": True, "ordinary_art": adopted,
                        "profile": "ui-usability-feedback", "evidence_kind": "feedback", "input_backend": "portal-eis",
                        "subject_commit": "historical-head", "source_fingerprint": "historical-source",
                        "sessions": [{"directory": "viewport-0", "case": loaded.art.CASE, "nonce": client["nonce"],
                                      "cancellation_sha256": loaded.pipeline.digest((session / "production-cancellation.json").read_bytes()),
                                      "log_sha256": loaded.pipeline.digest(log.read_bytes())}]}
            (root / "manifest.json").write_text(json.dumps(manifest))
            contract = copy.deepcopy(adopted)
            contract["resume_input"] = loaded.origin_contract(repo, root, contract)
            loaded.check_origin(repo, contract)
            destination = repo / "fresh"
            destination.mkdir()
            loaded.prepare_input(repo, destination, contract)
            with self.assertRaisesRegex(RuntimeError, "fresh"):
                loaded.prepare_input(repo, destination, contract)
            changed = copy.deepcopy(contract)
            changed["identity"]["generation"] += 1
            with self.assertRaisesRegex(RuntimeError, "candidate identity"):
                loaded.origin_contract(repo, root, changed)
            for path in (save, session / "capture-0.png", log):
                data = path.read_bytes()
                path.write_bytes(data + b"!")
                with self.assertRaises(RuntimeError):
                    loaded.check_origin(repo, contract)
                path.write_bytes(data)
            for field, value in (("status", "invalid"), ("child_pid", 123), ("failure", "prior failure")):
                changed = dict(job, **{field: value})
                (root / "job.json").write_text(json.dumps(changed))
                with self.assertRaisesRegex(RuntimeError, "ended candidate job"):
                    loaded.origin_contract(repo, root, contract)
            (root / "job.json").write_text(json.dumps(job))
            alias = repo / "target/native-acceptance/alias"
            alias.symlink_to(root, target_is_directory=True)
            with self.assertRaises(ValueError):
                loaded.origin_contract(repo, alias, contract)


if __name__ == "__main__":
    unittest.main()
