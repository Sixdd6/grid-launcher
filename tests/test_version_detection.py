from __future__ import annotations

import importlib
import sys
import unittest


class VersionDetectionTests(unittest.TestCase):
    def test_version_module_exposes_string(self) -> None:
        import grid_launcher.version as version

        importlib.reload(version)
        self.assertIsInstance(version.__version__, str)
        self.assertTrue(version.__version__)

    def test_package_reexports_version(self) -> None:
        import grid_launcher

        importlib.reload(grid_launcher)
        self.assertEqual(grid_launcher.__version__, grid_launcher.version.__version__)

    def test_default_dev_version(self) -> None:
        import grid_launcher.version as version

        importlib.reload(version)
        # Committed default when git describe has not overwritten version.py.
        self.assertEqual(version.__version__, "0.0.0-dev")

    def test_package_falls_back_when_version_missing(self) -> None:
        import grid_launcher

        saved = sys.modules.get("grid_launcher.version")
        # Setting the entry to None makes `import grid_launcher.version` raise ImportError.
        sys.modules["grid_launcher.version"] = None  # type: ignore[assignment]
        try:
            importlib.reload(grid_launcher)
            self.assertEqual(grid_launcher.__version__, "0.0.0-dev")
        finally:
            if saved is not None:
                sys.modules["grid_launcher.version"] = saved
            else:
                sys.modules.pop("grid_launcher.version", None)
            # Restore the real module for subsequent tests.
            importlib.reload(grid_launcher)


if __name__ == "__main__":
    unittest.main()

