from __future__ import annotations

import unittest

from scripts import check_repo_hygiene


class DotenvPathTests(unittest.TestCase):
    def test_local_dotenv_files_are_rejected(self) -> None:
        self.assertTrue(check_repo_hygiene.is_local_env_file(".env"))
        self.assertTrue(check_repo_hygiene.is_local_env_file("nested/.env.local"))
        self.assertTrue(check_repo_hygiene.is_local_env_file("nested/.env.production"))

    def test_example_and_unrelated_files_are_allowed(self) -> None:
        self.assertFalse(check_repo_hygiene.is_local_env_file(".env.example"))
        self.assertFalse(check_repo_hygiene.is_local_env_file("docs/project.env.local"))


if __name__ == "__main__":
    unittest.main()
