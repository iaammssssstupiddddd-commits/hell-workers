"""Recipe boundaries; deliberately does not execute native acceptance."""
import unittest
from scripts import building_bridge_acceptance as bridge


class BridgeAcceptanceTests(unittest.TestCase):
    def test_budget_must_be_explicit_positive_and_finite(self):
        good = {"frame_p95_ratio_max": 1.1, "frame_p99_ratio_max": 1.2,
                "rss_delta_bytes_max": 1000000, "native_bytes_delta_max": 1000000}
        self.assertFalse(bridge.performance(good)["accepted"])
        for invalid in (None, {}, {**good, "frame_p95_ratio_max": float("nan")},
                        {**good, "rss_delta_bytes_max": 0}, {**good, "native_bytes_delta_max": True}):
            with self.assertRaises(ValueError):
                bridge.performance(invalid)

    def test_independent_lifecycle_legs_and_disabled_performance_probe(self):
        self.assertEqual(set(bridge.LEGS), {"placement", "passage", "adjacent", "construction",
            "instant-build", "cancel", "save-load", "deconstruct", "non-movable", "gallery", "cleanup"})
        self.assertIn("normal UI", bridge.LEGS["adjacent"])
        self.assertIn("no fixture", bridge.LEGS["construction"])
        self.assertIn("River nonwalkability", bridge.LEGS["deconstruct"])


if __name__ == "__main__":
    unittest.main()
