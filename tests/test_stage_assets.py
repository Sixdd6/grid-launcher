import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

_spec = importlib.util.spec_from_file_location(
    "stage_assets", REPO_ROOT / "scripts" / "stage_assets.py"
)
stage_assets = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(stage_assets)

from grid_launcher.server.platform_metadata import PLATFORM_LOGO_FILES


class StageAssetsTests(unittest.TestCase):
    def test_retroarch_set_contains_every_platform_logo(self):
        derived = set(stage_assets.derive_retroarch_files())
        for logo_file in set(PLATFORM_LOGO_FILES.values()):
            self.assertIn(logo_file, derived)

    def test_gamepad_glyphs_are_derived_from_source_literals(self):
        derived = set(stage_assets.derive_retroarch_files())
        self.assertIn("input_BTN-R.png", derived)
        self.assertIn("input_DPAD-U.png", derived)
        self.assertIn("input_DPAD-D.png", derived)
        self.assertIn("input_DPAD-L.png", derived)
        self.assertIn("input_DPAD-R.png", derived)

    def test_gamepad_glyph_scan_ignores_lowercase_identifiers(self):
        source = 'ControlHint("Confirm", "input_BTN-D")\nself.input_widget = None\ninput_changed = 1\n'
        self.assertEqual(
            stage_assets.derive_gamepad_glyph_files(source), ["input_BTN-D.png"]
        )

    def test_every_derived_retroarch_file_exists(self):
        for name in stage_assets.derive_retroarch_files():
            self.assertTrue((stage_assets.ASSETS_ROOT / "retroarch-assets" / name).is_file(), name)

    def test_svg_set_contains_used_icon_and_excludes_unused_icon(self):
        derived = set(stage_assets.derive_svg_files())
        self.assertIn("play.svg", derived)
        self.assertNotIn("apps.svg", derived)
        self.assertNotIn("io.github.Sixdd6.GRIDLauncher.svg", derived)

    def test_svg_scan_normalizes_suffix_and_deduplicates(self):
        source = 'icon("svg/play")\nicon("svg/play.svg")\nicon("svg/star-outline.svg")\n'
        self.assertEqual(
            stage_assets.derive_svg_files(source), ["play.svg", "star-outline.svg"]
        )

    def test_every_derived_svg_file_exists(self):
        for name in stage_assets.derive_svg_files():
            self.assertTrue((stage_assets.ASSETS_ROOT / "svg" / name).is_file(), name)

    def test_7z_included_for_windows_and_excluded_for_linux(self):
        windows = [str(rel) for _, rel in stage_assets.plan_copies("windows")]
        linux = [str(rel) for _, rel in stage_assets.plan_copies("linux")]
        self.assertIn(str(Path("tools") / "7z" / "7z.exe"), windows)
        self.assertIn(str(Path("tools") / "7z" / "7z.dll"), windows)
        self.assertFalse([entry for entry in linux if entry.startswith("tools")])

    def test_stage_assets_recreates_output_with_expected_layout(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir) / "bundle-assets"
            stale_file = output_dir / "stale.png"
            stale_file.parent.mkdir(parents=True)
            stale_file.write_text("stale")

            copies = stage_assets.stage_assets(output_dir, "linux")

            self.assertFalse(stale_file.exists())
            self.assertTrue((output_dir / "svg" / "play.svg").is_file())
            self.assertTrue((output_dir / "retroarch-assets" / "input_BTN-R.png").is_file())
            self.assertFalse((output_dir / "tools").exists())
            self.assertEqual(sum(1 for path in output_dir.rglob("*") if path.is_file()), len(copies))


if __name__ == "__main__":
    unittest.main()
