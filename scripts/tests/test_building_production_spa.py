"""Spa raw messages remain observations, never whole-lifecycle approval."""
import copy
import unittest

from scripts import building_production_lifecycle as lifecycle


class SpaObservationTests(unittest.TestCase):
    def raw(self, cancel=True, result=None, payload=None):
        samples = [{"sample_index": i, "real_seconds": i + 1.0,
                    **{key: [] for key in lifecycle.TRANSITION_CATEGORIES}} for i in range(2)]
        event = {"producer": "SoulSpaConstructionCancelOutcome" if cancel else "SoulSpaSlotsChangeOutcome",
                 "operation": "SoulSpaConstructionCancel" if cancel else "SoulSpaSlotsChange",
                 "owner": "12v0", "result": result or "StaleTarget", "sample_index": 1,
                 "event_index": 0, "real_seconds": 2.0,
                 "refunded_bones" if cancel else "applied": payload}
        return {"schema_version": 2, "transition_contract": "indexed-state-delta-v1",
                "samples": samples, "events": [event]}

    def test_all_real_statuses_are_observations_not_lifecycle_witnesses(self):
        cases = [(True, status, None) for status in (
            "Paused", "StaleTarget", "PhaseUnavailable", "OwnerMismatch", "ActiveTaskMismatch")]
        cases += [(False, status, None) for status in ("StaleTarget", "UnsupportedTarget", "PhaseUnavailable")]
        cases += [(True, "Canceled { refunded_bones: 7 }", 7),
                  (True, "Canceled { refunded_bones: 0 }", 0),
                  (False, "Applied { requested: 99, applied: 4, clamped: true }",
                   {"requested": 99, "applied": 4, "clamped": True}),
                  (False, "Applied { requested: 0, applied: 0, clamped: false }",
                   {"requested": 0, "applied": 0, "clamped": False})]
        for cancel, status, payload in cases:
            raw = self.raw(cancel, status, payload)
            with self.subTest(status=status):
                lifecycle.check_raw_transitions(raw)
                with self.assertRaisesRegex(ValueError, "not an authoritative"):
                    lifecycle.check_domain_witness({"before": 0, "after": 1, "owner": "12v0",
                        "events": [0], "domain_event_indices": [0]}, raw)

    def test_malformed_envelopes_and_synthetic_success_rejected(self):
        for change in ({"producer": "Unknown"}, {"operation": "CancelConstruction"}, {"result": "Committed"},
                       {"owner": ""}, {"result": 1}, {"sample_index": True}, {"sample_index": -1},
                       {"sample_index": 2}, {"event_index": True}, {"event_index": -1},
                       {"real_seconds": True}, {"real_seconds": -1}, {"real_seconds": 3},
                       {"real_seconds": float("nan")}, {"accepted": True}, {"refunded_bones": 0}):
            raw = self.raw()
            raw["events"][0].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                lifecycle.check_raw_transitions(raw)

    def test_refund_payload_must_match_exact_bounded_typed_result(self):
        for status, payload in (("Canceled { refunded_bones: 7 }", True),
                                ("Canceled { refunded_bones: 7 }", 6),
                                ("Canceled { refunded_bones: 7 }", None),
                                ("Canceled { refunded_bones: -1 }", -1),
                                ("Canceled { refunded_bones: 4294967296 }", 2**32),
                                ("Canceled { refunded_bones: 7 } ignored", 7)):
            with self.subTest(status=status, payload=payload), self.assertRaises(ValueError):
                lifecycle.check_raw_transitions(self.raw(True, status, payload))

    def test_slots_payload_must_match_message_without_normalizing_observation(self):
        raw = self.raw(False, "Applied { requested: 99, applied: 4, clamped: true }",
                       {"requested": 99, "applied": 4, "clamped": True})
        for payload in (None, {"requested": True}, {"applied": 3}, {"clamped": 1},
                        {"clamped": False}, {"accepted": True}, {"requested": 2**32}):
            bad = copy.deepcopy(raw)
            if isinstance(payload, dict):
                bad["events"][0]["applied"].update(payload)
            else:
                bad["events"][0]["applied"] = payload
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                lifecycle.check_raw_transitions(bad)
        # Shape validation preserves an anomalous but faithfully recorded value;
        # no clamp, legal gameplay state, successful leg or cleanup is inferred.
        lifecycle.check_raw_transitions(self.raw(False,
            "Applied { requested: 99, applied: 8, clamped: false }",
            {"requested": 99, "applied": 8, "clamped": False}))


if __name__ == "__main__":
    unittest.main()
