from __future__ import annotations

import os
import unittest
from pathlib import Path
from unittest.mock import patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication

from grid_launcher.emulator.profiles import (
    _WINDOWS_ONLY_EMULATOR_SLUGS,
    is_available_on_current_platform,
)
from grid_launcher.ui.mixins.emulator_ui_mixin import EmulatorUIMixin
from grid_launcher.ui.mixins.install_mixin import InstallMixin
from grid_launcher.ui import dialogs as dialogs_module


# --- Step 5: is_available_on_current_platform() unit tests -------------------


class IsAvailableOnCurrentPlatformTests(unittest.TestCase):
    def test_windows_only_slugs_hidden_on_linux(self) -> None:
        for slug in (
            "Xenia Canary (Xbox 360)",
            "Xenia (Xbox 360)",
            "ShadPS4 Qt Launcher",
        ):
            with self.subTest(slug=slug):
                self.assertFalse(
                    is_available_on_current_platform({"name": slug}, platform="linux")
                )

    def test_windows_only_slugs_available_on_windows(self) -> None:
        for slug in _WINDOWS_ONLY_EMULATOR_SLUGS:
            with self.subTest(slug=slug):
                self.assertTrue(
                    is_available_on_current_platform({"name": slug}, platform="win32")
                )

    def test_slug_matching_is_case_insensitive(self) -> None:
        self.assertFalse(
            is_available_on_current_platform(
                {"name": "  XENIA CANARY (XBOX 360)  "}, platform="linux"
            )
        )

    def test_source_platforms_allowlist_gates_non_matching_platform(self) -> None:
        profile = {"name": "Some Windows Emu", "source": {"platforms": ["win32"]}}
        self.assertFalse(is_available_on_current_platform(profile, platform="linux"))
        self.assertFalse(is_available_on_current_platform(profile, platform="darwin"))
        self.assertTrue(is_available_on_current_platform(profile, platform="win32"))

    def test_source_platforms_allows_matching_platform(self) -> None:
        profile = {"name": "Linux Emu", "source": {"platforms": ["linux", "darwin"]}}
        self.assertTrue(is_available_on_current_platform(profile, platform="linux"))
        self.assertTrue(is_available_on_current_platform(profile, platform="darwin"))

    def test_cross_platform_profile_available_everywhere(self) -> None:
        profile = {
            "name": "Xenia Edge (Xbox 360)",
            "source": {"platform_overrides": {"linux": {"asset_patterns": ["x.AppImage"]}}},
        }
        self.assertTrue(is_available_on_current_platform(profile, platform="linux"))
        self.assertTrue(is_available_on_current_platform(profile, platform="win32"))

    def test_empty_source_platforms_does_not_gate(self) -> None:
        profile = {"name": "Regular Emu", "source": {"platforms": []}}
        self.assertTrue(is_available_on_current_platform(profile, platform="linux"))

    def test_non_dict_profile_is_available(self) -> None:
        self.assertTrue(is_available_on_current_platform("not-a-dict", platform="linux"))

    def test_windows_returns_true_for_all(self) -> None:
        for slug in _WINDOWS_ONLY_EMULATOR_SLUGS:
            self.assertTrue(
                is_available_on_current_platform({"name": slug}, platform="win32")
            )
        self.assertTrue(
            is_available_on_current_platform(
                {"name": "Any", "source": {"platforms": ["linux"]}}, platform="win32"
            )
        )

    def test_defaults_to_runtime_platform_when_unset(self) -> None:
        with patch("sys.platform", "linux"):
            self.assertFalse(
                is_available_on_current_platform({"name": "Xenia (Xbox 360)"})
            )
        with patch("sys.platform", "win32"):
            self.assertTrue(
                is_available_on_current_platform({"name": "Xenia (Xbox 360)"})
            )


# --- Step 6: EmulatorUIMixin._emulator_autoprofiles() filtering --------------


class _AutoprofileStubWindow(EmulatorUIMixin):
    def __init__(self, profiles: list[dict[str, object]]) -> None:
        self.emulator_autoprofiles = profiles

    def _normalize_save_strategy_value(self, value: object) -> str:
        return "auto"

    def _normalize_ignore_extension_value(self, value: object) -> str:
        return ""


_MIXED_PROFILES = [
    {"name": "RetroArch (Multi-System)", "match_tokens": ["retroarch.exe"]},
    {"name": "Xenia Edge (Xbox 360)", "match_tokens": ["xenia_edge_linux.AppImage"]},
    {"name": "Xenia Canary (Xbox 360)", "source": {"platforms": ["win32"]}},
    {"name": "Xenia (Xbox 360)", "match_tokens": ["xenia.exe"]},
    {"name": "ShadPS4 Qt Launcher", "source": {"platforms": ["win32"]}},
]


class EmulatorAutoprofileFilteringTests(unittest.TestCase):
    def _autoprofiles_for_platform(self, platform: str) -> list[str]:
        window = _AutoprofileStubWindow(list(_MIXED_PROFILES))
        with patch("sys.platform", platform), patch(
            "grid_launcher.ui.mixins.emulator_ui_mixin.resolve_emulator_autoprofiles",
            side_effect=lambda value, *_args, **_kwargs: value,
        ):
            return [profile["name"] for profile in window._emulator_autoprofiles()]

    def test_windows_only_profiles_removed_on_linux(self) -> None:
        names = self._autoprofiles_for_platform("linux")
        self.assertIn("RetroArch (Multi-System)", names)
        self.assertIn("Xenia Edge (Xbox 360)", names)
        self.assertNotIn("Xenia Canary (Xbox 360)", names)
        self.assertNotIn("Xenia (Xbox 360)", names)
        self.assertNotIn("ShadPS4 Qt Launcher", names)

    def test_all_profiles_present_on_windows(self) -> None:
        names = self._autoprofiles_for_platform("win32")
        for profile in _MIXED_PROFILES:
            self.assertIn(profile["name"], names)

    def test_full_cache_preserved_after_filtering(self) -> None:
        window = _AutoprofileStubWindow(list(_MIXED_PROFILES))
        with patch("sys.platform", "linux"), patch(
            "grid_launcher.ui.mixins.emulator_ui_mixin.resolve_emulator_autoprofiles",
            side_effect=lambda value, *_args, **_kwargs: value,
        ):
            window._emulator_autoprofiles()
        # The unfiltered cache still holds every profile for manual lookups.
        cached_names = [profile["name"] for profile in window.emulator_autoprofiles]
        self.assertIn("Xenia (Xbox 360)", cached_names)


# --- Step 7: EmulatorConfigDialog supported-profile filtering ----------------


class SupportedProfileFilteringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.app = QApplication.instance() or QApplication([])

    def test_windows_only_profiles_excluded_on_linux(self) -> None:
        with patch("sys.platform", "linux"):
            names = {
                row["name"] for row in dialogs_module._load_supported_emulator_profiles()
            }
        self.assertIn("Xenia Edge (Xbox 360)", names)
        self.assertNotIn("Xenia Canary (Xbox 360)", names)
        self.assertNotIn("Xenia (Xbox 360)", names)
        self.assertNotIn("ShadPS4 Qt Launcher", names)

    def test_windows_only_profiles_present_on_windows(self) -> None:
        with patch("sys.platform", "win32"):
            names = {
                row["name"] for row in dialogs_module._load_supported_emulator_profiles()
            }
        self.assertIn("Xenia Canary (Xbox 360)", names)
        self.assertIn("Xenia (Xbox 360)", names)
        self.assertIn("ShadPS4 Qt Launcher", names)


# --- Step 8: Xbox 360 install flow platform check ----------------------------


class _XboxInstallStub(InstallMixin):
    def __init__(self, emulator_name: str) -> None:
        self._emulator_name = emulator_name

    def _default_emulator_name_for_platform(self, platform: str) -> str:
        return self._emulator_name

    def _emulator_entry_by_name(self, name: str) -> dict[str, str]:
        return {"name": name, "path": "", "args": ""}

    def _split_launch_template_args(self, args: str) -> list[str]:
        return []


class Xbox360InstallPlatformCheckTests(unittest.TestCase):
    def test_linux_without_emulator_returns_clear_error(self) -> None:
        stub = _XboxInstallStub(emulator_name="")
        with patch("sys.platform", "linux"):
            game, message = stub._apply_xenia_content_archive_without_ui(
                {"platform": "Xbox 360"}, Path("content.zip")
            )
        self.assertIsNone(game)
        self.assertIn("Xenia Edge", message)

    def test_linux_with_windows_only_emulator_returns_clear_error(self) -> None:
        stub = _XboxInstallStub(emulator_name="Xenia (Xbox 360)")
        with patch("sys.platform", "linux"):
            game, message = stub._apply_xenia_content_archive_without_ui(
                {"platform": "Xbox 360"}, Path("content.zip")
            )
        self.assertIsNone(game)
        self.assertIn("only runs on Windows", message)

    def test_linux_with_compatible_emulator_passes_platform_gate(self) -> None:
        # Xenia Edge is Linux-compatible, so the platform gate must not trigger
        # and none of the gate error messages should be produced.
        stub = _XboxInstallStub(emulator_name="Xenia Edge (Xbox 360)")
        with patch("sys.platform", "linux"):
            _game, message = stub._apply_xenia_content_archive_without_ui(
                {"platform": "Xbox 360"}, Path("content.zip")
            )
        self.assertNotIn("Xenia Edge", message)
        self.assertNotIn("only runs on Windows", message)

    def test_windows_skips_platform_gate(self) -> None:
        stub = _XboxInstallStub(emulator_name="")
        with patch("sys.platform", "win32"):
            _game, message = stub._apply_xenia_content_archive_without_ui(
                {"platform": "Xbox 360"}, Path("content.zip")
            )
        self.assertNotIn("Xenia Edge", message)
        self.assertNotIn("only runs on Windows", message)


if __name__ == "__main__":
    unittest.main()
