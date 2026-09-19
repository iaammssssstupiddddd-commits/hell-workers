from __future__ import annotations

import copy
import unittest

from scripts import ci_result, ci_scope


def sample_plan():
    return {
        "schema_version": 1, "event_name": "push", "mode": "auto",
        "base_sha": "a" * 40, "head_sha": "b" * 40, "tested_sha": "b" * 40,
        "help_base_sha": "a" * 40, "diff_pairs": [["a" * 40, "b" * 40]],
        "groups": {"contracts": True, "tooling": True, "rust": True, "deps": False},
        "reason_codes": ["rust-with-tooling-contracts"], "path_count": 1,
        "paths_sha256": "c" * 64, "run_id": "42", "run_attempt": "1",
    }


def sample_needs(plan):
    outputs = {"tested_sha": plan["tested_sha"], "plan_sha256": ci_scope.plan_digest(plan)}
    needs = {"changes": {"result": "success", "outputs": outputs.copy()}}
    for job, group in ci_result.JOB_GROUPS.items():
        required = plan["groups"][group]
        needs[job] = {"result": "success" if required else "skipped", "outputs": outputs.copy() if required else {}}
    return needs


class AggregateTests(unittest.TestCase):
    def test_selected_success_and_unselected_skip_are_accepted(self):
        plan = sample_plan()
        self.assertEqual(ci_result.evaluate(plan, sample_needs(plan)), [])

    def test_required_failure_skip_cancel_missing_or_wrong_identity_are_rejected(self):
        plan = sample_plan()
        for job in ("changes", "contracts", "tooling", "rust"):
            for state in ("failure", "skipped", "cancelled", "neutral", None):
                needs = sample_needs(plan)
                needs[job]["result"] = state
                with self.subTest(job=job, state=state):
                    self.assertTrue(ci_result.evaluate(plan, needs))
            for key in ("tested_sha", "plan_sha256"):
                needs = sample_needs(plan)
                needs[job]["outputs"].pop(key)
                self.assertTrue(ci_result.evaluate(plan, needs))
        needs = sample_needs(plan)
        needs["rust"]["outputs"]["tested_sha"] = "d" * 40
        self.assertTrue(ci_result.evaluate(plan, needs))

    def test_unselected_success_and_missing_job_are_rejected(self):
        plan = sample_plan()
        needs = sample_needs(plan)
        needs["dependencies"]["result"] = "success"
        self.assertTrue(ci_result.evaluate(plan, needs))
        del needs["dependencies"]
        with self.assertRaises(ValueError):
            ci_result.evaluate(plan, needs)

    def test_invalid_schema_and_broken_group_closure_fail_closed(self):
        mutations = [
            lambda p: p["groups"].update(rust="false"),
            lambda p: p["groups"].update(tooling=False),
            lambda p: p["groups"].update(contracts=False),
            lambda p: p["groups"].update(unknown=True),
            lambda p: p.update(schema_version=True),
            lambda p: p.update(extra="value"),
            lambda p: p.update(mode="full"),
            lambda p: p.update(base_sha="0" * 40),
            lambda p: p.update(diff_pairs=[]),
            lambda p: p.update(run_id=""),
        ]
        for mutate in mutations:
            plan = sample_plan()
            mutate(plan)
            with self.assertRaises(ValueError):
                ci_scope.validate_plan(plan)

    def test_attempt_change_is_not_a_new_plan_but_revision_change_is(self):
        plan = sample_plan()
        retry = copy.deepcopy(plan)
        retry["run_attempt"] = "2"
        self.assertEqual(ci_scope.plan_digest(plan), ci_scope.plan_digest(retry))
        self.assertEqual(ci_result.evaluate(retry, sample_needs(plan)), [])
        retry["tested_sha"] = "d" * 40
        self.assertNotEqual(ci_scope.plan_digest(plan), ci_scope.plan_digest(retry))

    def test_duplicate_json_keys_and_dependency_only_event_cannot_certify_quality(self):
        with self.assertRaises(ValueError):
            ci_scope.load_json('{"groups":{},"groups":{}}')
        plan = sample_plan()
        plan["event_name"] = "schedule"
        with self.assertRaises(ValueError):
            ci_scope.validate_plan(plan)


if __name__ == "__main__":
    unittest.main()
