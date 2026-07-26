from __future__ import annotations

import io
import tempfile
import unittest
from pathlib import Path

from scripts import sync_agent_skills


class SkillSyncTests(unittest.TestCase):
    def test_expected_target_preserves_adapter_frontmatter(self) -> None:
        target = (
            "---\nname: adapter-name\n"
            "description: Adapter description\n---\n\nold body\n"
        )
        expected = sync_agent_skills.expected_target(target, "canonical body\n")
        self.assertEqual(
            expected,
            "---\nname: adapter-name\n"
            "description: Adapter description\n---\n\ncanonical body\n",
        )

    def test_split_frontmatter_rejects_missing_boundary(self) -> None:
        with self.assertRaises(ValueError):
            sync_agent_skills.split_frontmatter("name: invalid\n")

    def test_frontmatter_requires_valid_metadata_and_expected_name(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing Skill frontmatter"):
            sync_agent_skills.validate_skill_frontmatter(
                "---\nname: expected\n---\n\nbody\n",
                "expected",
            )
        with self.assertRaisesRegex(ValueError, "unsupported Skill description"):
            sync_agent_skills.validate_skill_frontmatter(
                "---\nname: expected\ndescription: [broken\n---\n\nbody\n",
                "expected",
            )
        with self.assertRaisesRegex(ValueError, "unexpected Skill name"):
            sync_agent_skills.validate_skill_frontmatter(
                "---\nname: wrong\ndescription: Valid description\n---\n\nbody\n",
                "expected",
            )

    def test_multiple_skill_groups_check_write_and_preserve_frontmatter(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            canonical_docs = root / "cursor-docs.md"
            codex_docs = root / "codex-docs.md"
            gemini_docs = root / "gemini-docs.md"
            canonical_help = root / "cursor-help.md"
            claude_help = root / "claude-help.md"

            canonical_docs.write_text(
                "---\nname: canonical-docs\n"
                "description: Canonical docs\n---\n\ncanonical docs body\n",
                encoding="utf-8",
            )
            codex_docs.write_text(
                "---\nname: codex-docs\n"
                "description: Codex docs\n---\n\nstale\n",
                encoding="utf-8",
            )
            gemini_docs.write_text(
                "---\nname: gemini-docs\n"
                "description: Gemini docs\n---\n\nstale\n",
                encoding="utf-8",
            )
            canonical_help.write_text(
                "---\nname: canonical-help\n"
                "description: Canonical help\n---\n\ncanonical help body\n",
                encoding="utf-8",
            )
            claude_help.write_text(
                "---\nname: claude-help\n"
                "description: Claude help\n---\n\nstale\n",
                encoding="utf-8",
            )
            skill_syncs = (
                sync_agent_skills.SkillSync(
                    sync_agent_skills.SkillDocument(
                        canonical_docs,
                        "canonical-docs",
                    ),
                    (
                        sync_agent_skills.SkillDocument(
                            codex_docs,
                            "codex-docs",
                        ),
                        sync_agent_skills.SkillDocument(
                            gemini_docs,
                            "gemini-docs",
                        ),
                    ),
                ),
                sync_agent_skills.SkillSync(
                    sync_agent_skills.SkillDocument(
                        canonical_help,
                        "canonical-help",
                    ),
                    (
                        sync_agent_skills.SkillDocument(
                            claude_help,
                            "claude-help",
                        ),
                    ),
                ),
            )

            stale = sync_agent_skills.sync_skills(
                skill_syncs,
                write=False,
                output=io.StringIO(),
                error=io.StringIO(),
            )
            self.assertEqual(set(stale), {codex_docs, gemini_docs, claude_help})
            self.assertIn("stale", codex_docs.read_text(encoding="utf-8"))

            sync_agent_skills.sync_skills(
                skill_syncs,
                write=True,
                output=io.StringIO(),
                error=io.StringIO(),
            )
            self.assertEqual(
                codex_docs.read_text(encoding="utf-8"),
                "---\nname: codex-docs\n"
                "description: Codex docs\n---\n\ncanonical docs body\n",
            )
            self.assertEqual(
                gemini_docs.read_text(encoding="utf-8"),
                "---\nname: gemini-docs\n"
                "description: Gemini docs\n---\n\ncanonical docs body\n",
            )
            self.assertEqual(
                claude_help.read_text(encoding="utf-8"),
                "---\nname: claude-help\n"
                "description: Claude help\n---\n\ncanonical help body\n",
            )
            self.assertEqual(
                sync_agent_skills.sync_skills(
                    skill_syncs,
                    write=False,
                    output=io.StringIO(),
                    error=io.StringIO(),
                ),
                (),
            )

    def test_missing_canonical_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            missing_canonical = root / "missing-canonical.md"
            with self.assertRaisesRegex(
                FileNotFoundError,
                "canonical Agent Skill is missing",
            ):
                sync_agent_skills.sync_skills(
                    (
                        sync_agent_skills.SkillSync(
                            sync_agent_skills.SkillDocument(
                                missing_canonical,
                                "missing-canonical",
                            ),
                            (),
                        ),
                    ),
                    write=False,
                    output=io.StringIO(),
                    error=io.StringIO(),
                )

    def test_write_preflight_failure_does_not_partially_update(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            first_canonical = root / "first-canonical.md"
            first_target = root / "first-target.md"
            second_canonical = root / "second-canonical.md"
            missing_target = root / "missing-target.md"
            first_canonical.write_text(
                "---\nname: first\ndescription: First\n---\n\nnew first body\n",
                encoding="utf-8",
            )
            first_target.write_text(
                "---\nname: first-adapter\n"
                "description: First adapter\n---\n\nold first body\n",
                encoding="utf-8",
            )
            second_canonical.write_text(
                "---\nname: second\ndescription: Second\n---\n\nnew second body\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                FileNotFoundError,
                "Agent Skill adapter is missing",
            ):
                sync_agent_skills.sync_skills(
                    (
                        sync_agent_skills.SkillSync(
                            sync_agent_skills.SkillDocument(
                                first_canonical,
                                "first",
                            ),
                            (
                                sync_agent_skills.SkillDocument(
                                    first_target,
                                    "first-adapter",
                                ),
                            ),
                        ),
                        sync_agent_skills.SkillSync(
                            sync_agent_skills.SkillDocument(
                                second_canonical,
                                "second",
                            ),
                            (
                                sync_agent_skills.SkillDocument(
                                    missing_target,
                                    "missing-target",
                                ),
                            ),
                        ),
                    ),
                    write=True,
                    output=io.StringIO(),
                    error=io.StringIO(),
                )

            self.assertEqual(
                first_target.read_text(encoding="utf-8"),
                "---\nname: first-adapter\n"
                "description: First adapter\n---\n\nold first body\n",
            )

    def test_missing_adapter_and_duplicate_paths_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            canonical = root / "canonical.md"
            canonical.write_text(
                "---\nname: canonical\ndescription: Canonical\n---\n\nbody\n",
                encoding="utf-8",
            )
            missing = root / "missing.md"
            canonical_document = sync_agent_skills.SkillDocument(
                canonical,
                "canonical",
            )
            skill_sync = sync_agent_skills.SkillSync(
                canonical_document,
                (
                    sync_agent_skills.SkillDocument(
                        missing,
                        "missing",
                    ),
                ),
            )

            with self.assertRaisesRegex(
                FileNotFoundError,
                "Agent Skill adapter is missing",
            ):
                sync_agent_skills.sync_skills(
                    (skill_sync,),
                    write=False,
                    output=io.StringIO(),
                    error=io.StringIO(),
                )

            with self.assertRaisesRegex(
                ValueError,
                "duplicate Agent Skill sync path",
            ):
                sync_agent_skills.sync_skills(
                    (
                        sync_agent_skills.SkillSync(canonical_document, ()),
                        sync_agent_skills.SkillSync(canonical_document, ()),
                    ),
                    write=False,
                    output=io.StringIO(),
                    error=io.StringIO(),
                )

    def test_codex_metadata_requires_ui_fields_and_skill_invocation(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            metadata_path = Path(temp_dir) / "openai.yaml"
            metadata = sync_agent_skills.CodexSkillMetadata(
                metadata_path,
                "example-skill",
            )
            metadata_path.write_text(
                'interface:\n'
                '  display_name: "Example Skill"\n'
                '  short_description: "Review and update the example workflow"\n'
                '  default_prompt: "Use $example-skill to review this change."\n',
                encoding="utf-8",
            )
            sync_agent_skills.validate_codex_metadata((metadata,))

            metadata_path.write_text(
                'interface:\n'
                '  display_name: "Example Skill"\n'
                '  short_description: "Review and update the example workflow"\n'
                '  default_prompt: "Review this change."\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                ValueError,
                r"default_prompt must mention \$example-skill",
            ):
                sync_agent_skills.validate_codex_metadata((metadata,))


if __name__ == "__main__":
    unittest.main()
