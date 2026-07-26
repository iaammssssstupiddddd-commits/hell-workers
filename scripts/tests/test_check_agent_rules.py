from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts import check_agent_rules


EXPECTED_ROOT_RULE_FILES = {
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursorrules",
    ".kilocoderules",
    ".github/copilot-instructions.md",
    ".gemini/antigravity/project_rules.md",
}
EXPECTED_MANDATORY_HELP_REVIEW_FILES = EXPECTED_ROOT_RULE_FILES | {
    ".agent/rules/help-impact.md",
    ".cursor/rules/help-impact.mdc",
    ".agent/workflows/task-lifecycle.md",
    ".cursor/workflows/task-lifecycle.md",
}


class MandatoryHelpReviewRuleTests(unittest.TestCase):
    def test_missing_rule_is_detected_without_flagging_compliant_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            compliant = root / "compliant.md"
            missing = root / "missing.md"
            compliant.write_text(
                "# Rules\n\n"
                f"{check_agent_rules.MANDATORY_HELP_REVIEW_RULE}\n"
                f"{check_agent_rules.MANDATORY_HELP_REVIEW_DECISION_RULE}\n"
                f"{check_agent_rules.MANDATORY_HELP_REVIEW_FALLBACK_RULE}\n",
                encoding="utf-8",
            )
            missing.write_text("# Rules\n\nRun tests.\n", encoding="utf-8")
            absent = root / "absent.md"

            self.assertEqual(
                check_agent_rules.missing_mandatory_help_review_rules(
                    (compliant, missing, absent)
                ),
                (missing, absent),
            )

    def test_every_active_rule_surface_requires_help_review(self) -> None:
        self.assertEqual(
            set(check_agent_rules.ROOT_RULE_FILES),
            EXPECTED_ROOT_RULE_FILES,
        )
        self.assertEqual(
            set(check_agent_rules.MANDATORY_HELP_REVIEW_FILES),
            EXPECTED_MANDATORY_HELP_REVIEW_FILES,
        )
        mandatory_rules = tuple(
            check_agent_rules.REPO_ROOT / relative
            for relative in check_agent_rules.MANDATORY_HELP_REVIEW_FILES
        )
        self.assertEqual(
            check_agent_rules.missing_mandatory_help_review_rules(
                mandatory_rules
            ),
            (),
        )


if __name__ == "__main__":
    unittest.main()
