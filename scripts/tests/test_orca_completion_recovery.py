from __future__ import annotations

import json
import shlex
import tempfile
import unittest
from pathlib import Path

from scripts import orca_completion_recovery as recovery


class RecordedCompletionTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.path = Path(temp.name) / "session.jsonl"
        self.client = Path("/controlled/bridge/p/orca")
        self.authority = {"task": "task_current", "dispatch": "ctx_current"}
        self.argv = [str(self.client), "orchestration", "send", "--from", "term_worker",
                     "--dispatch-capability", "dcap_fixture_only", "--type", "worker_done", "--subject", "Done",
                     "--body", "Added test. Scope inspected. Validation is pending.", "--task-id", "task_current",
                     "--dispatch-id", "ctx_current", "--outcome", "succeeded", "--files-modified", "scripts/tests/test.py", "--json"]

    def write(self, duplicate=False):
        row = {"type": "response_item", "payload": {"type": "custom_tool_call", "call_id": "call_exact",
               "input": "const r = await tools.exec_command(" + json.dumps({"cmd": shlex.join(self.argv)}) + ");text(r.output);"}}
        self.path.write_text((json.dumps(row) + "\n") * (2 if duplicate else 1))

    def read(self):
        return recovery.recorded_command(self.path, "call_exact", self.client, self.authority, "term_worker")

    def test_only_exact_recorded_completion_is_reconstructed_as_argv(self):
        self.write()
        argv, params = self.read()
        self.assertEqual(argv, self.argv[1:-1])
        self.assertEqual(json.loads(params["payload"])["filesModified"], ["scripts/tests/test.py"])
        self.assertTrue(params["waitForLifecycleSettlement"])

    def test_different_dispatch_is_refused(self):
        self.argv[self.argv.index("--dispatch-id") + 1] = "ctx_other"
        self.write()
        with self.assertRaisesRegex(ValueError, "identity"):
            self.read()

    def test_extra_shell_input_is_not_executed(self):
        self.argv[-1:-1] = [";", "unexpected"]
        self.write()
        with self.assertRaises(ValueError):
            self.read()

    def test_ambiguous_record_is_refused(self):
        self.write(duplicate=True)
        with self.assertRaisesRegex(ValueError, "exact recorded"):
            self.read()


if __name__ == "__main__":
    unittest.main()
