from __future__ import annotations

import os
import unittest
from unittest.mock import patch

import grid_launcher.core.process as process_module
from grid_launcher.core.process import clean_subprocess_env


class CleanSubprocessEnvTests(unittest.TestCase):
    def test_restores_saved_loader_path_when_present(self) -> None:
        env = {
            "LD_LIBRARY_PATH": "/bundle/_internal",
            "LD_LIBRARY_PATH_ORIG": "/usr/lib64",
        }
        cleaned = clean_subprocess_env(env)
        self.assertEqual(cleaned["LD_LIBRARY_PATH"], "/usr/lib64")

    def test_drops_loader_path_when_frozen_without_saved_value(self) -> None:
        env = {"LD_LIBRARY_PATH": "/bundle/_internal", "PATH": "/usr/bin"}
        with patch.object(process_module.sys, "frozen", True, create=True):
            cleaned = clean_subprocess_env(env)
        self.assertNotIn("LD_LIBRARY_PATH", cleaned)
        self.assertEqual(cleaned["PATH"], "/usr/bin")

    def test_leaves_env_untouched_when_not_frozen(self) -> None:
        env = {"LD_LIBRARY_PATH": "/custom/libs"}
        cleaned = clean_subprocess_env(env)
        self.assertEqual(cleaned["LD_LIBRARY_PATH"], "/custom/libs")

    def test_does_not_mutate_input_mapping(self) -> None:
        env = {
            "LD_LIBRARY_PATH": "/bundle/_internal",
            "LD_LIBRARY_PATH_ORIG": "/usr/lib64",
        }
        clean_subprocess_env(env)
        self.assertEqual(env["LD_LIBRARY_PATH"], "/bundle/_internal")

    def test_defaults_to_os_environ(self) -> None:
        with patch.dict(os.environ, {"GRID_TEST_SENTINEL": "1"}, clear=False):
            cleaned = clean_subprocess_env()
        self.assertEqual(cleaned.get("GRID_TEST_SENTINEL"), "1")


if __name__ == "__main__":
    unittest.main()
