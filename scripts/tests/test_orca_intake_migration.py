from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import (
    host_coordination,
    orca_coordinator,
    orca_frontdesk,
    orca_intake_migration as migration,
    orca_issue_context,
)


WORKSPACE = "eeac8301-ddb2-4c31-8a6c-e2a7f2fc7efb"
ISSUE_ID = "884ddcd2-cef6-4869-a88d-14512684cce7"
RUNTIME = "e52da240-994e-4496-81e3-dbaf29967035"


def response(description: str = "Imported replacement") -> dict:
    return {
        "id": "3ce87fa3-30d7-4074-aad2-acf0c7cde95b",
        "ok": True,
        "result": {
            "issue": {
                "id": ISSUE_ID,
                "identifier": "HW-42",
                "title": "Migrate intake",
                "description": description,
                "url": "https://linear.app/hell-workers/issue/HW-42/migrate-intake",
                "updatedAt": "2026-09-21T08:30:00.000Z",
                "state": {"name": "Todo", "type": "unstarted"},
                "priorityLabel": "High",
                "labels": [],
                "team": {"workspace": {"id": WORKSPACE}},
            },
            "inlineMedia": [],
            "meta": {
                "workspaceId": WORKSPACE,
                "includeErrors": [],
                "sections": {"comments": {"capReached": False}},
            },
        },
        "_meta": {"runtimeId": RUNTIME},
    }


class IntakeMigrationTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="intake-migration-", dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.cli = self.root / "orca-cli"
        self.cli.write_text("#!/bin/sh\nexit 1\n")
        self.cli.chmod(0o700)
        for module in (
            host_coordination,
            orca_frontdesk,
            orca_issue_context,
            migration,
        ):
            mock = patch.object(
                module, "state_root", return_value=self.root / "coordination"
            )
            mock.start()
            self.addCleanup(mock.stop)

    def completed(self, value: dict | None = None):
        return subprocess.CompletedProcess(
            args=[], returncode=0, stdout=json.dumps(value or response()), stderr=""
        )

    def import_linear(self, description: str = "Imported replacement") -> dict:
        with patch.object(
            orca_issue_context.subprocess,
            "run",
            return_value=self.completed(response(description)),
        ):
            return orca_issue_context.import_issue("HW-42", WORKSPACE, self.cli)

    def test_link_is_durable_idempotent_and_preserves_both_requests(self) -> None:
        legacy = orca_frontdesk.submit("legacy request")
        linear = self.import_linear()

        first = migration.link_requests(legacy["id"], linear["request_id"])
        second = migration.link_requests(legacy["id"], linear["request_id"])

        self.assertEqual(first, second)
        self.assertEqual(
            {item["id"] for item in orca_frontdesk.list_requests()},
            {legacy["id"], linear["request_id"]},
        )
        self.assertEqual(migration.mapping_for_legacy(legacy["id"]), first)
        self.assertIsNone(migration.mapping_for_legacy(linear["request_id"]))
        self.assertEqual(migration.ledger_path().stat().st_mode & 0o777, 0o600)

    def test_migrated_legacy_consult_is_refused_before_provider_but_state_is_readable(
        self,
    ) -> None:
        legacy = orca_frontdesk.submit("legacy request")
        linear = self.import_linear()
        migration.link_requests(legacy["id"], linear["request_id"])

        with (
            patch.object(orca_coordinator, "provider_command") as provider,
            self.assertRaisesRegex(migration.IntakeMigrationError, "migrated"),
        ):
            orca_coordinator.consult(legacy["id"])
        provider.assert_not_called()
        state = orca_coordinator.read_state(legacy["id"])
        self.assertEqual(state["request_id"], legacy["id"])
        self.assertEqual(state["turns"], [])

    def test_link_requires_one_manual_source_and_one_imported_linear_target(
        self,
    ) -> None:
        manual_a = orca_frontdesk.submit("manual a")
        manual_b = orca_frontdesk.submit("manual b")
        linear = self.import_linear()
        cases = [
            (manual_a["id"], manual_b["id"]),
            (linear["request_id"], manual_a["id"]),
            (linear["request_id"], linear["request_id"]),
            ("e1fd2684-d55a-4794-9741-903c92b7dbea", linear["request_id"]),
        ]
        for legacy_id, linear_id in cases:
            with (
                self.subTest(legacy=legacy_id, linear=linear_id),
                self.assertRaises(migration.IntakeMigrationError),
            ):
                migration.link_requests(legacy_id, linear_id)
        self.assertEqual(migration.read_ledger(migration.ledger_path())["mappings"], [])

    def test_one_to_one_mapping_refuses_aliases_without_changing_first_record(
        self,
    ) -> None:
        legacy_a = orca_frontdesk.submit("legacy a")
        legacy_b = orca_frontdesk.submit("legacy b")
        linear_a = self.import_linear("replacement a")
        linear_b = self.import_linear("replacement b")
        first = migration.link_requests(legacy_a["id"], linear_a["request_id"])

        for legacy_id, linear_id in (
            (legacy_a["id"], linear_b["request_id"]),
            (legacy_b["id"], linear_a["request_id"]),
        ):
            with self.assertRaisesRegex(migration.IntakeMigrationError, "elsewhere"):
                migration.link_requests(legacy_id, linear_id)
        self.assertEqual(
            migration.read_ledger(migration.ledger_path())["mappings"], [first]
        )

    def test_corrupt_or_duplicate_mapping_is_not_reset(self) -> None:
        legacy = orca_frontdesk.submit("legacy")
        linear = self.import_linear()
        record = migration.link_requests(legacy["id"], linear["request_id"])
        path = migration.ledger_path()
        raw = json.dumps({"schema": 1, "mappings": [record, record]})
        path.write_text(raw)
        with self.assertRaises(migration.IntakeMigrationError):
            migration.mapping_for_legacy(legacy["id"])
        self.assertEqual(path.read_text(), raw)

    def test_busy_migration_lock_refuses_without_partial_write(self) -> None:
        legacy = orca_frontdesk.submit("legacy")
        linear = self.import_linear()
        with host_coordination.acquire_host("linear-intake-state", inherit=False):
            with self.assertRaises(host_coordination.HostBusyError):
                migration.link_requests(legacy["id"], linear["request_id"])
        self.assertEqual(migration.read_ledger(migration.ledger_path())["mappings"], [])


if __name__ == "__main__":
    unittest.main()
