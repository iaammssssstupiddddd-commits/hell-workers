"""Synthetic UI predicates do not grant art, numeric, or release approval."""
import copy
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from scripts.tests import test_ui_rest_area_art as ordinary_art
from scripts.tests.test_ui_rest_area_completion import acceptance

zoom = acceptance.rest_area_maxzoom


class MaxzoomTests(unittest.TestCase):
    def verification_fixture(self):
        first = self.observation()
        first["camera_numeric"][0]["scale"] = [1., 1., 1.]
        raw = {"nonce": first["nonce"], "process_id": first["pid"], "scope": zoom.art.SCOPE,
               "failure": None, "accepted": False, "performance_evidence": False, "promotion_authority": False, "samples": [], "events": []}
        events, wheels = [], []

        def sample(value, scale):
            state = value["rest_area_planning"]["rest_area_usage"]
            raw["samples"].append({"sample_index": len(raw["samples"]), "world_epoch": value["world_epoch"],
                                   "real_seconds": state["real_seconds"], "virtual_seconds": state["virtual_seconds"],
                                   "rest_area_context": copy.deepcopy(state), "paused": True, "camera_scale": scale,
                                   "play_mode": "Normal", "task_mode": "None", "menu_state": "Hidden",
                                   "primary_window": "75v0", "cursor_position": [500., 300.], "dpi": 1.})

        def append_event(value, **fields):
            events.append({"step": str(len(events) + 1), "nonce": value["nonce"], "pid": value["pid"], "window": 123,
                           "world_epoch": value["world_epoch"], **fields})

        first["rest_area_planning"]["rest_area_usage"].update(real_seconds=10., virtual_seconds=10.)
        sample(first, 1.)

        for index in range(40):
            value = self.observation()
            before, after = round(1 + index * .1, 6), round(1 + (index + 1) * .1, 6)
            value["camera_numeric"][0]["scale"] = [after] * 3
            value["frame"] = 205 + index * 5
            value["rest_area_planning"]["rest_area_usage"]["real_seconds"] = 20. + index
            value["rest_area_planning"]["rest_area_usage"]["virtual_seconds"] = 10.
            wheel = {"index": index, "window": 99, "epoch": value["world_epoch"], "frame": value["frame"] - 1,
                     "unit": "Line", "phase": "Moved", "delta": [0., -1.]}
            wheels.append(wheel)
            sample(value, after)
            append_event(value, scroll_discrete=[0, 120],
                           **{
                           "pre_frame": value["frame"] - 4, "post_frame": value["frame"],
                           "wheel_ack": wheel, "camera_ack": value, "wheel_raw_index": len(raw["samples"]) - 1,
                           "camera_before": before, "camera_after": after})
        entries = [{"case": case, "value": copy.deepcopy(first), "last_step": 0} for case in zoom.planning.COMPLETION_CHECKPOINTS]
        for index, case in enumerate(zoom.CASES):
            value = self.observation("-occupied-" in case)
            value["frame"] = 500 + index * 10
            value["rest_area_planning"]["rest_area_usage"]["real_seconds"] = 100. + index
            value["rest_area_planning"]["rest_area_usage"]["virtual_seconds"] = 10. if index < 2 else 20.
            value["wheel_events"] = wheels
            if index == 2:
                unpaused = copy.deepcopy(value)
                unpaused["rest_area_planning"]["rest_area_usage"].update(real_seconds=101.5, virtual_seconds=15.)
                sample(unpaused, 5.)
                raw["samples"][-1]["paused"] = False
            if index % 2 == 0:
                append_event(value, point=[500, 300], pre_frame=value["frame"] - 5, post_frame=value["frame"] - 1)
            else:
                for offset, pressed in enumerate((True, False)):
                    append_event(value, button=1, pressed=pressed, observed_left_pressed=pressed,
                                 pre_frame=value["frame"] - 9 + offset * 3, post_frame=value["frame"] - 7 + offset * 3)
                    click = copy.deepcopy(value)
                    click["rest_area_planning"]["rest_area_usage"]["real_seconds"] -= .6 - offset * .2
                    sample(click, 5.)
                    raw["events"].append({"producer": "MouseButtonInput", "operation": "MouseButtonInput", "button": "Left",
                                          "state": "Pressed" if pressed else "Released", "window": "75v0",
                                          "event_index": len(raw["events"]), "sample_index": len(raw["samples"]) - 1,
                                          "real_seconds": raw["samples"][-1]["real_seconds"]})
            sample(value, 5.)
            entries.append({"case": case, "value": value, "last_step": len(events)})
        return raw, entries, {"eis_scroll": True, "nonce": first["nonce"], "pid": first["pid"], "window": 123}, {}, events

    def verify_wheel_fixture(self, fixture):
        # Only isolate the new wheel/UI verifier here; existing tests exercise
        # the real construction and art/raw validators with full domain data.
        with patch.object(zoom.planning, "check_completion"), patch.object(zoom.art, "check_consumers"):
            zoom.verify(*fixture)

    def test_wheel_chain_binds_actual_delta_raw_camera_and_final_ui(self):
        fixture = self.verification_fixture()
        self.verify_wheel_fixture(fixture)
        for mode in ("empty", "too_many", "capability", "direction", "fraction", "before", "after", "sample", "sample_bool",
                     "raw_camera", "raw_epoch", "ack_window", "ack_unit", "ack_delta", "ack_frame", "ack_duplicate",
                     "captured_nonce", "captured_camera", "final_camera", "owner", "sequence"):
            raw, entries, client, contract, events = copy.deepcopy(fixture)
            event = events[0]
            if mode == "empty":
                events.clear()
            elif mode == "too_many":
                events.extend([copy.deepcopy(event)] * 49)
            elif mode == "capability":
                client["eis_scroll"] = False
            elif mode in ("direction", "fraction", "before", "after", "sample", "sample_bool"):
                key, changed = {"direction": ("scroll_discrete", [0, -120]), "fraction": ("scroll_discrete", [False, 120]),
                    "before": ("camera_before", True), "after": ("camera_after", 1.5), "sample": ("wheel_raw_index", 999),
                    "sample_bool": ("wheel_raw_index", True)}[mode]
                event[key] = changed
            elif mode in ("raw_camera", "raw_epoch"):
                raw["samples"][event["wheel_raw_index"]]["camera_scale" if mode == "raw_camera" else "world_epoch"] = 7
            elif mode.startswith("ack_"):
                key, changed = {"ack_window": ("window", 100), "ack_unit": ("unit", "unknown"),
                    "ack_delta": ("delta", [0., 1.]), "ack_frame": ("frame", 1), "ack_duplicate": ("index", 1)}[mode]
                event["wheel_ack"][key] = changed
            elif mode == "captured_nonce":
                event["camera_ack"]["nonce"] = "foreign"
            elif mode == "captured_camera":
                event["camera_ack"]["camera_numeric"][0]["scale"] = [1.5] * 3
            elif mode == "final_camera":
                entries[-1]["value"]["camera_numeric"][0]["scale"] = [4.] * 3
            elif mode == "owner":
                entries[-1]["value"]["rest_area_planning"]["buildings"] = [999]
            elif mode == "sequence":
                entries.reverse()
            with self.subTest(mode=mode), self.assertRaises((RuntimeError, ValueError)):
                self.verify_wheel_fixture((raw, entries, client, contract, events))

    def test_declared_line_and_pixel_units_use_actual_pan_camera_conversion(self):
        for unit, delta in (("Line", -1), ("Pixel", -100)):
            self.assertAlmostEqual(zoom.zoom_after(1., {"unit": unit, "phase": "Moved", "delta": [0., delta]}), 1.1)
        for unit, delta in (("unknown", -1), ("Line", True), ("Line", 0), ("Pixel", float("nan"))):
            with self.subTest(unit=unit, delta=delta), self.assertRaises(RuntimeError):
                zoom.zoom_after(1., {"unit": unit, "phase": "Moved", "delta": [0., delta]})

    def test_temporal_and_actual_left_witness_corruption_fails_closed(self):
        for mode in ("frame", "step", "real_time", "virtual_time", "no_simulation", "motion", "click_removed", "click_moved",
                     "click_state", "click_nonce", "raw_removed", "raw_extra", "raw_order", "raw_window", "raw_epoch",
                     "raw_point", "raw_owner", "raw_time", "raw_interval", "raw_button", "checkpoint_pid",
                     "checkpoint_pid_bool", "checkpoint_nonce", "checkpoint_window"):
            raw, entries, client, contract, events = copy.deepcopy(self.verification_fixture())
            selected = entries[4]
            click = events[selected["last_step"] - 2]
            witness = raw["events"][0]
            row = raw["samples"][witness["sample_index"]]
            if mode == "frame":
                selected["value"]["frame"] = entries[3]["value"]["frame"]
            elif mode == "step":
                selected["last_step"] = entries[3]["last_step"]
            elif mode == "real_time":
                row["real_seconds"] = 99.
            elif mode == "virtual_time":
                row["virtual_seconds"] += 1
                row["rest_area_context"]["virtual_seconds"] += 1
            elif mode == "no_simulation":
                for sample in raw["samples"]:
                    sample["paused"] = True
            elif mode == "motion":
                events[entries[3]["last_step"] - 1]["point"] = [400, 300]
            elif mode == "click_removed":
                del events[selected["last_step"] - 2]
            elif mode == "click_moved":
                click.update(pre_frame=199, post_frame=200)
            elif mode == "click_state":
                click["observed_left_pressed"] = False
            elif mode == "click_nonce":
                click["nonce"] = "foreign"
            elif mode == "raw_removed":
                raw["events"].pop(0)
            elif mode == "raw_extra":
                raw["events"].append(copy.deepcopy(witness))
            elif mode == "raw_order":
                witness["event_index"] = raw["events"][1]["event_index"] + 1
            elif mode == "raw_window":
                witness["window"] = "76v0"
            elif mode == "raw_epoch":
                row["world_epoch"] += 1
            elif mode == "raw_point":
                row["cursor_position"] = [100, 300]
            elif mode == "raw_owner":
                row["rest_area_context"]["owners"] = []
            elif mode == "raw_time":
                witness["real_seconds"] = 0.
            elif mode == "raw_interval":
                witness["sample_index"] = 0
            elif mode == "raw_button":
                witness["button"] = "Right"
            elif mode == "checkpoint_pid":
                selected["pid"] = selected["value"]["pid"] = 999999
            elif mode == "checkpoint_pid_bool":
                selected["pid"] = selected["value"]["pid"] = True
            elif mode == "checkpoint_nonce":
                selected["value"]["nonce"] = "foreign"
            elif mode == "checkpoint_window":
                selected["value"]["primary_window"] = 100
                selected["value"]["camera_numeric"][0]["target_window"] = 100
            with self.subTest(mode=mode), self.assertRaises((RuntimeError, ValueError)):
                self.verify_wheel_fixture((raw, entries, client, contract, events))

    def test_checkpoint_boundary_never_uses_a_future_click_sample(self):
        raw, entries, _, _, _ = self.verification_fixture()
        value = entries[3]["value"]
        before = zoom.checkpoint_sample(raw["samples"], value)
        future = copy.deepcopy(before)
        future.update(sample_index=999, real_seconds=100.01)
        future["rest_area_context"]["real_seconds"] = 100.01
        value["rest_area_planning"]["rest_area_usage"]["real_seconds"] = 100.005
        self.assertEqual(zoom.checkpoint_sample([before, future], value)["sample_index"], before["sample_index"])
        with self.assertRaises(zoom.usage.RawCheckpointPending):
            zoom.checkpoint_sample([future], value)

    def observation(self, occupied=False):
        _, entries, _, _, _ = ordinary_art.OrdinaryArtTests().fixture()
        value = copy.deepcopy(entries[3]["value"])
        state = value["rest_area_planning"]["rest_area_usage"]
        if not occupied:
            state["owners"][0]["occupants"] = None
            for soul in state["souls"]:
                soul.update(resting_in=None, visibility="Inherited")
                soul["idle"]["behavior"] = "Wandering"
        rect = [0, 0, 200, 100]
        value.update(ready=True, frame=200, viewport=[1920, 1080], scale_factor=1.,
                     camera_numeric=[{"entity": 42, "active": True, "target_window": 99, "scale": [5., 5., 5.], "orthographic_scale": 1.}],
                     primary_window=99, hovered=8, pointer_target=8, tooltip_target=8,
                     tooltip_epoch=value["world_epoch"], wheel_overflow=False, wheel_events=[],
                     world_input_captured=False, pointer_over_ui=False, paused=True, cursor=[500, 300],
                     menu_state="Hidden", mode_presentation={"rect": None},
                     tooltip_alpha=1., tooltip_presentation={"rect": rect}, selected=8)
        value["rest_area_planning"]["owner_point"] = [500, 300]
        count = len(state["owners"][0]["occupants"] or [])
        value["tooltip_text"] = [{"text": "Building: RestArea", "alpha": 1., "rect": rect},
                                 {"text": f"Resting: {count}/4 | Dream: {count * .5:.2f}/s", "alpha": 1., "rect": rect}]
        value["controls"] = {"text:inspection-header": {"text": "Building: RestArea", "rect": rect, "fully_visible": True}}
        return value

    def test_real_owner_tooltip_and_selection_at_both_occupancy_states(self):
        for occupied in (False, True):
            value = self.observation(occupied)
            before = copy.deepcopy(value)
            for kind in ("hover", "selected"):
                zoom.check_observation(f"rest-area-maxzoom-{'occupied' if occupied else 'empty'}-{kind}", value)
            self.assertEqual(value, before)

    def test_wrong_camera_owner_tooltip_visibility_or_occupancy_fail_closed(self):
        for mode in ("camera_count", "inactive", "camera_window", "camera_scale", "camera_nan", "camera_bool",
                     "projection", "hover", "pointer", "tooltip_target", "epoch", "overflow", "captured", "ui",
                     "unpaused", "mode", "cursor", "tooltip_alpha", "text_alpha", "text_rect", "root_rect",
                     "tooltip_count", "tooltip_rate", "tooltip_kind", "selected", "header", "header_rect", "phase"):
            value = self.observation()
            camera = value["camera_numeric"][0]
            if mode == "camera_count":
                value["camera_numeric"].append(copy.deepcopy(camera))
            elif mode in ("inactive", "camera_window", "camera_scale", "camera_nan", "camera_bool", "projection"):
                key, changed = {"inactive": ("active", False), "camera_window": ("target_window", 100),
                    "camera_scale": ("scale", [4., 4., 4.]), "camera_nan": ("scale", [float("nan"), 5., 5.]),
                    "camera_bool": ("entity", True), "projection": ("orthographic_scale", None)}[mode]
                camera[key] = changed
            elif mode in ("hover", "pointer", "tooltip_target", "epoch", "overflow", "captured", "ui", "unpaused", "cursor", "tooltip_alpha", "selected"):
                key, changed = {"hover": ("hovered", 7), "pointer": ("pointer_target", 7), "tooltip_target": ("tooltip_target", 7),
                    "epoch": ("tooltip_epoch", 7), "overflow": ("wheel_overflow", True), "captured": ("world_input_captured", True),
                    "ui": ("pointer_over_ui", True), "unpaused": ("paused", False), "cursor": ("cursor", [501, 310]),
                    "tooltip_alpha": ("tooltip_alpha", .5), "selected": ("selected", 7)}[mode]
                value[key] = changed
            elif mode == "mode":
                value["mode_presentation"]["rect"] = [0, 0, 1, 1]
            elif mode in ("text_alpha", "text_rect", "tooltip_count", "tooltip_rate", "tooltip_kind"):
                key, changed = {"text_alpha": ("alpha", .4), "text_rect": ("rect", [-1, 0, 10, 10]),
                    "tooltip_count": ("text", "Resting: 1/4 | Dream: 0.50/s"),
                    "tooltip_rate": ("text", "Resting: 0/4 | Dream: 2.00/s"),
                    "tooltip_kind": ("text", "Building: SoulSpa")}[mode]
                value["tooltip_text"][0 if mode == "tooltip_kind" else 1][key] = changed
            elif mode == "root_rect":
                value["tooltip_presentation"]["rect"] = None
            elif mode in ("header", "header_rect"):
                value["controls"]["text:inspection-header"]["text" if mode == "header" else "rect"] = "foreign" if mode == "header" else None
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                zoom.check_observation("rest-area-maxzoom-occupied-selected" if mode == "phase" else "rest-area-maxzoom-empty-selected", value)

    def test_projection_applies_actual_scale_once_and_rejects_outside_or_missing(self):
        value = self.observation()
        value["scale_factor"] = 2.
        self.assertEqual(zoom.point(value), (1000, 600))
        value["rest_area_planning"]["owner_point"] = [2000, 100]
        with self.assertRaises(RuntimeError):
            zoom.point(value)
        value["rest_area_planning"]["owner_point"] = None
        with self.assertRaises(RuntimeError):
            zoom.point(value)

    def test_maxzoom_is_explicit_art_smoke_eis_only_and_hashed_in_all_harnesses(self):
        args = SimpleNamespace(rest_area_art_source="/candidate", art_codec="/codec", art_contract_sha256=None,
                               case="rest-area-maxzoom", smoke=True, input_backend="portal-eis", layout_menu=False)
        with patch.object(zoom.art, "source_contract", return_value={"identity": "fixture"}):
            self.assertEqual(zoom.art.planned_contract(args, Path("/repo")), {"identity": "fixture"})
            for key, changed in (("rest_area_art_source", None), ("smoke", False), ("input_backend", "portal"), ("layout_menu", True)):
                invalid = copy.copy(args)
                setattr(invalid, key, changed)
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    zoom.art.planned_contract(invalid, Path("/repo"))
        from scripts.perf_tool import execution, renderdoc_capture
        for inventory in (zoom.native.NATIVE_HARNESS_FILES, execution.MEASUREMENT_HARNESS_FILES, renderdoc_capture.MEASUREMENT_HARNESS_FILES):
            self.assertEqual(inventory.count(".codex/skills/hell-workers-run-native-acceptance/scripts/ui_rest_area_maxzoom.py"), 1)


if __name__ == "__main__":
    unittest.main()
