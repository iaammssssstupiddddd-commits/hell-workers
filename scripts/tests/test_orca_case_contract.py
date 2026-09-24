from __future__ import annotations

import unittest

from scripts import orca_case_contract as contract


WORKFLOW = {
    "id": "4252a97e-396b-4779-8ed8-a032f6f0dcda",
    "revision": "1-abcd1234abcd",
    "title": "保存領域を直す",
    "kind": "task",
    "state": "unknown",
    "detail": "統括タブの所在を確認できません。作業場は保持してあります。",
    "actions": [],
    "roles": [],
}


class CaseContractTests(unittest.TestCase):
    def test_schema_one_preview_is_non_destructive_and_visible(self) -> None:
        legacy = {"schema": 1, "runtimeId": "runtime", "publishedAt": 42,
                  "workflows": [WORKFLOW]}
        upgraded = contract.migrate_preview(legacy)
        self.assertEqual(legacy["schema"], 1)
        self.assertEqual(upgraded["schema"], 2)
        self.assertEqual(upgraded["workflows"][0]["stage"], "attention")
        self.assertIn("再照合", upgraded["workflows"][0]["nextAction"])
        self.assertEqual(upgraded["workflows"][0]["observedAt"], 42)

    def test_unknown_schema_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "unsupported"):
            contract.migrate_preview({"schema": 99})

    def test_decision_and_external_sync_are_digest_bound(self) -> None:
        decision = {"disposition": "continue_existing", "rationale": "same objective",
                    "existingIssue": {"identifier": "TAK-14"}, "parentIssue": None,
                    "sha256": "a" * 64}
        result = contract.enrich(WORKFLOW, observed_at=100, decision=decision,
                                 sync={"state": "offline", "detail": "Linear再接続待ち"})
        self.assertEqual(result["intakeDecision"]["issueIdentifier"], "TAK-14")
        self.assertEqual(result["externalSync"]["state"], "offline")


if __name__ == "__main__":
    unittest.main()
