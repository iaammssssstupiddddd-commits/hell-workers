from __future__ import annotations

import unittest

from scripts import orca_reconciler as reconciler


BASE = {"dirtySource": False, "headChanged": False, "unknownProcess": False,
        "storageReady": True, "runtimeReady": True, "externalOnline": True,
        "snapshotFresh": True, "controllerRunning": True, "mutationInFlight": False,
        "terminalPresent": True, "processExited": False, "settledReceipt": False,
        "sessionConnected": True, "sameSession": True, "receiptAcknowledged": True,
        "bridgeAccepted": True}


class ReconcilerTests(unittest.TestCase):
    def diagnosis(self, **changes) -> dict:
        return reconciler.diagnose({**BASE, **changes})

    def test_stale_projection_is_the_only_state_changed_by_republish(self) -> None:
        result = self.diagnosis(snapshotFresh=False)
        self.assertEqual(result["action"], "republish_snapshot")
        called = []
        self.assertEqual(reconciler.repair(result, {"republish_snapshot": lambda: called.append(1)}), None)
        self.assertEqual(called, [1])

    def test_dirty_or_unknown_process_never_auto_repairs(self) -> None:
        for changes in ({"dirtySource": True}, {"unknownProcess": True},
                        {"controllerRunning": False, "mutationInFlight": True}):
            with self.subTest(changes=changes):
                result = self.diagnosis(**changes)
                self.assertEqual(result["disposition"], "maintenance")
                with self.assertRaisesRegex(ValueError, "does not authorize"):
                    reconciler.repair(result, {})

    def test_same_session_reconnect_and_settled_exit_are_safe(self) -> None:
        self.assertEqual(self.diagnosis(sessionConnected=False)["action"], "reconnect_same_session")
        self.assertEqual(self.diagnosis(terminalPresent=False, processExited=True,
                                        settledReceipt=True)["action"], "finalize_exited_terminal")

    def test_external_outage_waits_without_blocking_known_local_state(self) -> None:
        result = self.diagnosis(externalOnline=False)
        self.assertEqual(result["code"], "external_offline")
        self.assertEqual(result["disposition"], "wait")


if __name__ == "__main__":
    unittest.main()
