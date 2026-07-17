from __future__ import annotations

import unittest

from scripts import sync_agent_skills


class SkillSyncTests(unittest.TestCase):
    def test_expected_target_preserves_adapter_frontmatter(self) -> None:
        target = "---\nname: adapter-name\n---\n\nold body\n"
        expected = sync_agent_skills.expected_target(target, "canonical body\n")
        self.assertEqual(
            expected,
            "---\nname: adapter-name\n---\n\ncanonical body\n",
        )

    def test_split_frontmatter_rejects_missing_boundary(self) -> None:
        with self.assertRaises(ValueError):
            sync_agent_skills.split_frontmatter("name: invalid\n")


if __name__ == "__main__":
    unittest.main()
