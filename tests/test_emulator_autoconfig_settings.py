from __future__ import annotations

import importlib.util
import os
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from unittest.mock import patch, ANY, MagicMock
import json

from grid_launcher.core.path import xdg_config_home
from grid_launcher.emulator.azahar import (
    azahar_config_path_candidates,
    azahar_user_root_candidates,
    ensure_azahar_settings,
)
from grid_launcher.emulator.cemu import cemu_settings_path_candidates, ensure_cemu_controller_config, ensure_cemu_settings
from grid_launcher.emulator import ensure_dolphin_settings, ensure_dolphin_skip_ipl, ensure_dolphin_gcpad_config
from grid_launcher.emulator.dolphin import dolphin_ini_path_candidates, dolphin_user_root_candidates
from grid_launcher.emulator.duckstation import ensure_duckstation_memory_card_settings
from grid_launcher.emulator.eden import _ensure_eden_section_values, ensure_eden_settings, eden_config_path_candidates
from grid_launcher.emulator.launch import retroarch_core_argument_path
from grid_launcher.emulator.pico8 import pico8_user_root_candidates
from grid_launcher.emulator.pcsx2 import (
    ensure_pcsx2_settings,
    pcsx2_config_path_candidates,
    pcsx2_data_root_candidates,
)
from grid_launcher.emulator.ppsspp import ensure_ppsspp_settings
from grid_launcher.emulator.rpcs3 import (
    copy_ps3_custom_config_to_emulator,
    ensure_rpcs3_settings,
    rpcs3_data_root,
    rpcs3_data_root_candidates,
    update_rpcs3_games_yml,
    ps3_vfs_games_path,
)
from grid_launcher.emulator.redream import ensure_redream_settings
from grid_launcher.emulator.retroarch import ensure_retroarch_save_location_settings
from grid_launcher.emulator.xemu import ensure_xemu_settings, xemu_base_path_candidates
from grid_launcher.emulator import xemu_missing_bios_files
from grid_launcher.ui.mixins.emulator_ui_mixin import EmulatorUIMixin


class EmulatorAutoConfigSettingsTests(unittest.TestCase):
    def setUp(self) -> None:
        self._home_tempdir = tempfile.TemporaryDirectory()
        self.addCleanup(self._home_tempdir.cleanup)
        home_dir = Path(self._home_tempdir.name) / "home"
        home_dir.mkdir(parents=True, exist_ok=True)
        patcher = patch("pathlib.Path.home", return_value=home_dir)
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_retroarch_writes_missing_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text("", encoding="utf-8")

            result = ensure_retroarch_save_location_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn('audio_volume = "-18.000000"', text)
        self.assertIn('discord_enable = "false"', text)
        self.assertIn('video_windowed_fullscreen = "true"', text)
        self.assertIn('savestate_auto_save = "false"', text)

    def test_retroarch_preserves_user_audio_volume(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text('audio_volume = "-6.000000"\n', encoding="utf-8")

            ensure_retroarch_save_location_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn('audio_volume = "-6.000000"', text)
        self.assertNotIn('audio_volume = "-18.000000"', text)

    def test_retroarch_writes_netplay_nickname_when_username_given(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text("", encoding="utf-8")

            ensure_retroarch_save_location_settings(str(emulator_path), username="testuser")
            text = config_path.read_text(encoding="utf-8")

        self.assertIn('netplay_nickname = "testuser"', text)

    def test_retroarch_skips_netplay_nickname_when_no_username(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            retroarch_dir = Path(temp_dir) / "RetroArch"
            retroarch_dir.mkdir()
            emulator_path = retroarch_dir / "retroarch.exe"
            emulator_path.write_bytes(b"")
            config_path = retroarch_dir / "retroarch.cfg"
            config_path.write_text("", encoding="utf-8")

            ensure_retroarch_save_location_settings(str(emulator_path), username="")
            text = config_path.read_text(encoding="utf-8")

        self.assertNotIn("netplay_nickname", text)

    def test_duckstation_writes_inhibit_screensaver(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text("", encoding="utf-8")

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Main]", text)
        self.assertIn("InhibitScreensaver = true", text)

    def test_duckstation_preserves_user_confirm_poweroff(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text("[Main]\nConfirmPowerOff = true\n", encoding="utf-8")

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("ConfirmPowerOff = true", text)
        self.assertNotIn("ConfirmPowerOff = false", text)

    def test_duckstation_creates_portable_txt(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "DuckStation"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "duckstation.exe"
            exe_path.write_bytes(b"")

            ensure_duckstation_memory_card_settings(str(exe_path))
            portable_exists = (emulator_dir / "portable.txt").exists()

        self.assertTrue(portable_exists)

    def test_duckstation_does_not_overwrite_existing_portable_txt(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "DuckStation"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "duckstation.exe"
            exe_path.write_bytes(b"")
            portable_path = emulator_dir / "portable.txt"
            portable_path.write_text("custom", encoding="utf-8")

            ensure_duckstation_memory_card_settings(str(exe_path))
            portable_content = portable_path.read_text(encoding="utf-8")

        self.assertEqual("custom", portable_content)

    def test_duckstation_config_written_to_portable_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "DuckStation"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "duckstation.exe"
            exe_path.write_bytes(b"")

            result = ensure_duckstation_memory_card_settings(str(exe_path))
            config_path = result["config_path"]

        self.assertEqual(emulator_dir / "settings.ini", Path(str(config_path)))

    def test_pcsx2_creates_ini_when_absent(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            result = ensure_pcsx2_settings(str(emulator_path))
            config_path = Path(result["config_path"])
            config_exists = config_path.exists()
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertTrue(config_exists)
        self.assertIn("[EmuCore/GS]", text)
        self.assertIn("pcrtc_antiblur = true", text)
        self.assertIn("[EmuCore]", text)
        self.assertIn("EnableDiscordPresence = false", text)

    def test_pcsx2_ensure_creates_portable_ini(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PCSX2"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "pcsx2-qt.exe"
            exe_path.write_bytes(b"")

            ensure_pcsx2_settings(str(exe_path))

            portable_path = emulator_dir / "portable.ini"
            portable_exists = portable_path.exists()

        self.assertTrue(portable_exists)

    def test_pcsx2_ensure_does_not_overwrite_existing_portable_ini(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PCSX2"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "pcsx2-qt.exe"
            exe_path.write_bytes(b"")
            portable_path = emulator_dir / "portable.ini"
            portable_path.write_text("custom", encoding="utf-8")

            ensure_pcsx2_settings(str(exe_path))

            portable_content = portable_path.read_text(encoding="utf-8")

        self.assertEqual("custom", portable_content)

    def test_pcsx2_ensure_config_at_portable_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PCSX2"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "pcsx2-qt.exe"
            exe_path.write_bytes(b"")

            result = ensure_pcsx2_settings(str(exe_path))

            config_path = result["config_path"]

        self.assertEqual(emulator_dir / "inis" / "PCSX2.ini", config_path)

    def test_pcsx2_preserves_existing_confirm_shutdown(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("[UI]\nConfirmShutdown = true\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("ConfirmShutdown = true", text)
        self.assertNotIn("ConfirmShutdown = false", text)

    def test_duckstation_writes_cheevos_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text("", encoding="utf-8")

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Cheevos]", text)
        self.assertIn("Enabled = true", text)
        self.assertIn("ChallengeMode = false", text)
        self.assertIn("LeaderboardNotifications = false", text)
        self.assertIn("LeaderboardTrackers = false", text)

    def test_pcsx2_writes_fullscreen_when_enabled(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            (pcsx2_dir / "portable.ini").write_text("", encoding="utf-8")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path), enable_fullscreen=True)
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("StartFullscreen = true", text)

    def test_pcsx2_skips_fullscreen_when_disabled(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertNotIn("StartFullscreen", text)

    def test_pcsx2_skips_retroachievements_when_no_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertNotIn("[Achievements]", text)

    def test_pcsx2_writes_bios_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            (pcsx2_dir / "portable.ini").write_text("", encoding="utf-8")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path), bios_directory="/some/bios/path")
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Folders]", text)
        self.assertIn("Bios = /some/bios/path", text)

    def test_pcsx2_skips_bios_directory_when_empty(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertNotIn("Bios =", text)

    def test_pcsx2_writes_default_pad1_controller(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            (pcsx2_dir / "portable.ini").write_text("", encoding="utf-8")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Pad1]", text)
        self.assertIn("Type = DualShock2", text)
        self.assertIn("Cross = SDL-0/FaceSouth", text)
        self.assertIn("Circle = SDL-0/FaceEast", text)
        self.assertIn("Triangle = SDL-0/FaceNorth", text)
        self.assertIn("Square = SDL-0/FaceWest", text)
        self.assertIn("LUp = SDL-0/-LeftY", text)

    def test_pcsx2_preserves_user_pad1_controller(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("[Pad1]\nType = DigitalController\nCross = Keyboard/Z\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("Type = DigitalController", text)
        self.assertNotIn("Type = DualShock2", text)

    def test_pcsx2_writes_pause_menu_hotkey(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            (pcsx2_dir / "portable.ini").write_text("", encoding="utf-8")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Hotkeys]", text)
        self.assertIn("OpenPauseMenu = SDL-0/Guide", text)

    def test_pcsx2_preserves_user_pause_menu_hotkey(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("[Hotkeys]\nOpenPauseMenu = Keyboard/Escape\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("OpenPauseMenu = Keyboard/Escape", text)

    def test_pcsx2_writes_default_standard_volume(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            (pcsx2_dir / "portable.ini").write_text("", encoding="utf-8")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[SPU2/Output]", text)
        self.assertIn("StandardVolume = 40", text)

    def test_pcsx2_preserves_user_standard_volume(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("[SPU2/Output]\nStandardVolume = 80\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("StandardVolume = 80", text)
        self.assertNotIn("StandardVolume = 40", text)

    def test_pcsx2_writes_default_upscale_multiplier(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            (pcsx2_dir / "portable.ini").write_text("", encoding="utf-8")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("upscale_multiplier = 3", text)

    def test_pcsx2_preserves_user_upscale_multiplier(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            pcsx2_dir = Path(temp_dir) / "PCSX2"
            pcsx2_dir.mkdir()
            emulator_path = pcsx2_dir / "pcsx2-qt.exe"
            emulator_path.write_bytes(b"")
            config_path = pcsx2_dir / "inis" / "PCSX2.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("[EmuCore/GS]\nupscale_multiplier = 5\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                ensure_pcsx2_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("upscale_multiplier = 5", text)

    def test_pcsx2_data_root_candidates_includes_windows_documents_folder(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            fake_docs = Path(temp_dir) / "MyDocs"
            fake_docs.mkdir()
            with patch("grid_launcher.emulator.pcsx2._windows_documents_folder", return_value=fake_docs):
                candidates = pcsx2_data_root_candidates("", "", lambda s: [])
        self.assertIn(fake_docs / "PCSX2", candidates)

    def test_pcsx2_data_root_candidates_includes_flatpak_path(self) -> None:
        candidates = pcsx2_data_root_candidates("", "", lambda s: [])
        self.assertIn(Path.home() / ".var" / "app" / "net.pcsx2.PCSX2" / "config" / "PCSX2", candidates)

    def test_pcsx2_config_path_candidates_includes_flatpak_path(self) -> None:
        candidates = pcsx2_config_path_candidates("/nonexistent/pcsx2-qt.exe")
        self.assertIn(
            Path.home() / ".var" / "app" / "net.pcsx2.PCSX2" / "config" / "PCSX2" / "inis" / "PCSX2.ini",
            candidates,
        )

    def test_pcsx2_windows_documents_folder_exported_from_module(self) -> None:
        from grid_launcher.emulator import pcsx2_windows_documents_folder

        self.assertTrue(callable(pcsx2_windows_documents_folder))

    def test_pcsx2_autoprofile_has_no_documents_or_appdata_tokens(self) -> None:
        import json

        profile_path = Path(__file__).resolve().parents[1] / "emulator-autoprofiles.json"
        with open(profile_path, encoding="utf-8") as f:
            profiles = json.load(f)

        pcsx2_profile = next(
            (p for p in profiles if any("pcsx2-qt.exe" in str(t).lower() for t in p.get("match_tokens", []))),
            None,
        )
        self.assertIsNotNone(pcsx2_profile)
        save_dirs = pcsx2_profile.get("save_directories", [])
        state_dirs = pcsx2_profile.get("state_directories", [])
        screenshot_dirs = pcsx2_profile.get("screenshot_directories", [])

        for directory in [*save_dirs, *state_dirs, *screenshot_dirs]:
            self.assertNotIn("%DOCUMENTS%", directory)
            self.assertNotIn("%APPDATA%", directory)

        self.assertIn("memcards", save_dirs)
        self.assertIn("sstates", state_dirs)
        self.assertIn("snaps", screenshot_dirs)
        self.assertFalse(
            any("%USERPROFILE%" in d for d in state_dirs),
            f"Old %USERPROFILE% still present in state_directories: {state_dirs}",
        )

    def test_dolphin_writes_both_ini_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_settings(str(emulator_path))
            dolphin_ini = Path(str(result["dolphin_ini_path"]))
            gfx_ini = Path(str(result["gfx_ini_path"]))
            dolphin_exists = dolphin_ini.exists()
            gfx_exists = gfx_ini.exists()

            dolphin_text = dolphin_ini.read_text(encoding="utf-8")
            gfx_text = gfx_ini.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertTrue(dolphin_exists)
        self.assertTrue(gfx_exists)
        self.assertIn("[Display]", dolphin_text)
        self.assertIn("Fullscreen = True", dolphin_text)
        self.assertIn("[Settings]", gfx_text)
        self.assertIn("UseVerticalSync = True", gfx_text)

    def test_dolphin_creates_portable_txt(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "Dolphin"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "dolphin.exe"
            exe_path.write_bytes(b"")

            ensure_dolphin_settings(str(exe_path))
            portable_exists = (emulator_dir / "portable.txt").exists()

        self.assertTrue(portable_exists)

    def test_dolphin_does_not_overwrite_existing_portable_txt(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "Dolphin"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "dolphin.exe"
            exe_path.write_bytes(b"")
            portable_path = emulator_dir / "portable.txt"
            portable_path.write_text("custom", encoding="utf-8")

            ensure_dolphin_settings(str(exe_path))
            portable_content = portable_path.read_text(encoding="utf-8")

        self.assertEqual("custom", portable_content)

    def test_dolphin_user_root_candidates_includes_flatpak_path(self) -> None:
        candidates = dolphin_user_root_candidates("", "", lambda s: [])
        self.assertIn(Path.home() / ".var" / "app" / "org.DolphinEmu.dolphin-emu" / "data" / "dolphin-emu", candidates)

    def test_dolphin_ini_path_candidates_includes_flatpak_path(self) -> None:
        candidates = dolphin_ini_path_candidates("/nonexistent/dolphin.exe", "Dolphin.ini")
        self.assertIn(
            Path.home() / ".var" / "app" / "org.DolphinEmu.dolphin-emu" / "data" / "dolphin-emu" / "Dolphin.ini",
            candidates,
        )

    def test_dolphin_analytics_both_keys_written(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_settings(str(emulator_path))
            dolphin_ini = Path(str(result["dolphin_ini_path"]))
            dolphin_text = dolphin_ini.read_text(encoding="utf-8")

        self.assertIn("[Analytics]", dolphin_text)
        self.assertIn("Enabled = False", dolphin_text)
        self.assertIn("PermissionAsked = True", dolphin_text)

    def test_dolphin_skip_ipl_creates_ini_with_skip_ipl_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_skip_ipl(str(emulator_path))
            dolphin_ini = Path(str(result["dolphin_ini_path"]))
            dolphin_exists = dolphin_ini.exists()
            dolphin_text = dolphin_ini.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertTrue(dolphin_exists)
        self.assertIn("[Core]", dolphin_text)
        self.assertIn("SkipIPL = False", dolphin_text)

    def test_dolphin_skip_ipl_changes_true_to_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            dolphin_ini = dolphin_dir / "User" / "Config" / "Dolphin.ini"
            dolphin_ini.parent.mkdir(parents=True, exist_ok=True)
            dolphin_ini.write_text("[Core]\nSkipIPL = True\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_skip_ipl(str(emulator_path))
            dolphin_text = dolphin_ini.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("SkipIPL = False", dolphin_text)

    def test_dolphin_skip_ipl_no_write_when_already_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            dolphin_ini = dolphin_dir / "User" / "Config" / "Dolphin.ini"
            dolphin_ini.parent.mkdir(parents=True, exist_ok=True)
            dolphin_ini.write_text("[Core]\nSkipIPL = False\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_skip_ipl(str(emulator_path))

        self.assertFalse(result["changed"])
        self.assertIsNotNone(result["dolphin_ini_path"])

    def test_dolphin_skip_ipl_preserves_other_settings(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            dolphin_ini = dolphin_dir / "User" / "Config" / "Dolphin.ini"
            dolphin_ini.parent.mkdir(parents=True, exist_ok=True)
            dolphin_ini.write_text(
                "[Core]\nSkipIPL = True\nCPUThread = True\n\n[Display]\nFullscreen = True\n",
                encoding="utf-8",
            )

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_skip_ipl(str(emulator_path))
            dolphin_text = dolphin_ini.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("SkipIPL = False", dolphin_text)
        self.assertIn("CPUThread = True", dolphin_text)
        self.assertIn("[Display]", dolphin_text)
        self.assertIn("Fullscreen = True", dolphin_text)

    def test_dolphin_skip_ipl_with_empty_path_uses_appdata_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            appdata_dir = Path(temp_dir) / "appdata"
            with patch.dict(os.environ, {"APPDATA": str(appdata_dir)}, clear=False):
                with patch("grid_launcher.emulator.dolphin.Path.home", return_value=Path(temp_dir) / "home"):
                    result = ensure_dolphin_skip_ipl("")
        self.assertTrue(str(result["dolphin_ini_path"]).startswith(str(appdata_dir)))

    def test_dolphin_gcpad_creates_ini_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_gcpad_config(str(emulator_path))

            gcpad_ini = Path(str(result["gcpad_ini_path"]))
            gcpad_text = gcpad_ini.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIsNotNone(result["gcpad_ini_path"])
        self.assertIn("[GCPad1]", gcpad_text)
        self.assertIn("Device = XInput/0/Gamepad", gcpad_text)
        self.assertIn("Buttons/A = `Button A`", gcpad_text)

    def test_dolphin_gcpad_skips_when_section_exists(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            gcpad_ini = dolphin_dir / "User" / "Config" / "GCPadNew.ini"
            gcpad_ini.parent.mkdir(parents=True, exist_ok=True)
            gcpad_ini.write_text(
                "[GCPad1]\n"
                "Device = DInput/0/Custom Controller\n"
                "Buttons/A = `Button 1`\n",
                encoding="utf-8",
            )

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_gcpad_config(str(emulator_path))

            gcpad_text = gcpad_ini.read_text(encoding="utf-8")

        self.assertFalse(result["changed"])
        self.assertIn("Device = DInput/0/Custom Controller", gcpad_text)

    def test_dolphin_gcpad_skips_when_section_exists_different_case(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            gcpad_ini = dolphin_dir / "User" / "Config" / "GCPadNew.ini"
            gcpad_ini.parent.mkdir(parents=True, exist_ok=True)
            gcpad_ini.write_text("[gcpad1]\nDevice = DInput/0/Custom Controller\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_gcpad_config(str(emulator_path))

        self.assertFalse(result["changed"])

    def test_dolphin_gcpad_appends_when_file_exists_without_gcpad1(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            dolphin_dir = Path(temp_dir) / "Dolphin"
            dolphin_dir.mkdir()
            emulator_path = dolphin_dir / "dolphin.exe"
            emulator_path.write_bytes(b"")
            gcpad_ini = dolphin_dir / "User" / "Config" / "GCPadNew.ini"
            gcpad_ini.parent.mkdir(parents=True, exist_ok=True)
            gcpad_ini.write_text("[GCPad2]\nDevice = XInput/1/Gamepad\n", encoding="utf-8")

            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_dolphin_gcpad_config(str(emulator_path))

            gcpad_text = gcpad_ini.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[GCPad2]", gcpad_text)
        self.assertIn("[GCPad1]", gcpad_text)
        self.assertIn("Device = XInput/1/Gamepad", gcpad_text)

    def test_dolphin_gcpad_with_empty_path_uses_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            appdata_dir = Path(temp_dir) / "appdata"
            with patch.dict(os.environ, {"APPDATA": str(appdata_dir)}, clear=False):
                with patch("grid_launcher.emulator.dolphin.Path.home", return_value=Path(temp_dir) / "home"):
                    result = ensure_dolphin_gcpad_config("")

        self.assertTrue(str(result["gcpad_ini_path"]).startswith(str(appdata_dir)))

    def test_azahar_writes_renderer_settings(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            azahar_dir = Path(temp_dir) / "Azahar"
            azahar_dir.mkdir()
            emulator_path = azahar_dir / "azahar.exe"
            emulator_path.write_bytes(b"")
            config_path = azahar_dir / "user" / "config" / "qt-config.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            ensure_azahar_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Renderer]", text)
        self.assertIn("resolution_factor = 4", text)
        self.assertIn("resolution_factor\\default = false", text)
        self.assertIn("use_vsync = true", text)

    def test_azahar_writes_audio_volume(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            azahar_dir = Path(temp_dir) / "Azahar"
            azahar_dir.mkdir()
            emulator_path = azahar_dir / "azahar.exe"
            emulator_path.write_bytes(b"")
            config_path = azahar_dir / "user" / "config" / "qt-config.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            ensure_azahar_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Audio]", text)
        self.assertIn("volume = 0.4", text)
        self.assertIn("volume\\default = false", text)

    def test_azahar_writes_fullscreen_and_ui_settings(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            azahar_dir = Path(temp_dir) / "Azahar"
            azahar_dir.mkdir()
            emulator_path = azahar_dir / "azahar.exe"
            emulator_path.write_bytes(b"")
            config_path = azahar_dir / "user" / "config" / "qt-config.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            ensure_azahar_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[UI]", text)
        self.assertIn("fullscreen = true", text)
        self.assertIn("fullscreen\\default = false", text)
        self.assertIn("pauseWhenInBackground = true", text)
        self.assertIn("hideInactiveMouse = true", text)

    def test_azahar_writes_shortcut_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            azahar_dir = Path(temp_dir) / "Azahar"
            azahar_dir.mkdir()
            emulator_path = azahar_dir / "azahar.exe"
            emulator_path.write_bytes(b"")
            config_path = azahar_dir / "user" / "config" / "qt-config.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            ensure_azahar_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("Shortcuts\\Main%20Window\\Fullscreen\\KeySeq\\default = false", text)
        self.assertIn("Shortcuts\\Main%20Window\\Fullscreen\\KeySeq = F1", text)
        self.assertIn("Shortcuts\\Main%20Window\\Stop%20Emulation\\KeySeq\\default = false", text)
        self.assertIn("Shortcuts\\Main%20Window\\Stop%20Emulation\\KeySeq = Escape", text)

    def test_azahar_companion_keys_not_duplicated(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            azahar_dir = Path(temp_dir) / "Azahar"
            azahar_dir.mkdir()
            emulator_path = azahar_dir / "azahar.exe"
            emulator_path.write_bytes(b"")
            config_path = azahar_dir / "user" / "config" / "qt-config.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            ensure_azahar_settings(str(emulator_path))
            ensure_azahar_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertEqual(1, text.count("resolution_factor\\default"))

    def test_azahar_creates_user_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "Azahar"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "azahar.exe"
            exe_path.write_bytes(b"")

            ensure_azahar_settings(str(exe_path))
            user_dir_exists = (emulator_dir / "user").is_dir()

        self.assertTrue(user_dir_exists)

    def test_azahar_does_not_clear_existing_user_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "Azahar"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "azahar.exe"
            exe_path.write_bytes(b"")
            marker_path = emulator_dir / "user" / "marker.txt"
            marker_path.parent.mkdir(parents=True, exist_ok=True)
            marker_path.write_text("x", encoding="utf-8")

            ensure_azahar_settings(str(exe_path))
            marker_content = marker_path.read_text(encoding="utf-8")

        self.assertEqual("x", marker_content)

    def test_azahar_user_root_candidates_includes_flatpak_path(self) -> None:
        candidates = azahar_user_root_candidates("")
        self.assertIn(Path.home() / ".var" / "app" / "org.azahar_emu.Azahar" / "data" / "Azahar", candidates)

    def test_azahar_config_path_candidates_includes_flatpak_path(self) -> None:
        candidates = azahar_config_path_candidates("/nonexistent/azahar.exe")
        self.assertIn(
            Path.home() / ".var" / "app" / "org.azahar_emu.Azahar" / "config" / "Azahar" / "qt-config.ini",
            candidates,
        )

    def test_eden_config_path_candidates_honors_xdg_config_home_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xdg_config_home = Path(temp_dir) / "xdg-config"
            with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(xdg_config_home)}, clear=False):
                candidates = eden_config_path_candidates("/nonexistent/eden.exe")

        self.assertIn(xdg_config_home / "eden" / "qt-config.ini", candidates)

    def test_eden_config_path_candidates_falls_back_to_dotfile_dir_when_xdg_unset(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("XDG_CONFIG_HOME", None)
            candidates = eden_config_path_candidates("/nonexistent/eden.exe")

        self.assertIn(Path.home() / ".config" / "eden" / "qt-config.ini", candidates)

    def test_eden_writes_telemetry_and_discord(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")
            config_path = eden_dir / "user" / "config" / "qt-config.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("", encoding="utf-8")

            ensure_eden_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[UI]", text)
        self.assertIn("enable_discord_presence\\default=false", text)
        self.assertIn("enable_discord_presence = false", text)
        self.assertIn("confirmStop\\default=false", text)
        self.assertIn("confirmStop = 2", text)
        self.assertIn("fullscreen\\default=false", text)
        self.assertIn("fullscreen = true", text)
        self.assertIn("firstStart\\default=false", text)
        self.assertIn("firstStart = false", text)
        self.assertIn("pauseWhenInBackground\\default=false", text)
        self.assertIn("pauseWhenInBackground = true", text)
        self.assertIn("enable_gamemode\\default=false", text)
        self.assertIn("enable_gamemode = true", text)
        self.assertIn("theme\\default=false", text)
        self.assertIn("theme = colorful_dark", text)
        self.assertIn("check_for_updates\\default=false", text)
        self.assertIn("check_for_updates = false", text)
        self.assertIn("[WebService]", text)
        self.assertIn("enable_telemetry\\default=false", text)
        self.assertIn("enable_telemetry = false", text)
        self.assertIn("[Audio]", text)
        self.assertIn("volume\\default=false", text)
        self.assertIn("volume = 40", text)
        self.assertIn("muteWhenInBackground\\default=false", text)
        self.assertIn("muteWhenInBackground = true", text)
        self.assertIn("[Renderer]", text)
        self.assertIn("scaling_filter\\default=false", text)
        self.assertIn("scaling_filter = 6", text)

    def test_eden_section_values_writes_annotation_and_value(self) -> None:
        result, changed = _ensure_eden_section_values("", "UI", {"confirmStop": "2"})
        self.assertTrue(changed)
        self.assertIn("[UI]", result)
        self.assertIn("confirmStop = 2", result)
        # annotation must be present and appear before the value
        annotation = "confirmStop\\default=false"
        self.assertIn(annotation, result)
        self.assertLess(result.index(annotation), result.index("confirmStop = 2"))

    def test_eden_creates_user_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "Eden"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "eden.exe"
            exe_path.write_bytes(b"")

            ensure_eden_settings(str(exe_path))
            user_dir_exists = (emulator_dir / "user").is_dir()

        self.assertTrue(user_dir_exists)

    def test_eden_does_not_clear_existing_user_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "Eden"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "eden.exe"
            exe_path.write_bytes(b"")
            marker_path = emulator_dir / "user" / "marker.txt"
            marker_path.parent.mkdir(parents=True, exist_ok=True)
            marker_path.write_text("x", encoding="utf-8")

            ensure_eden_settings(str(exe_path))
            marker_content = marker_path.read_text(encoding="utf-8")

        self.assertEqual("x", marker_content)

    def test_eden_confirm_stop_is_2_not_confirm_before_closing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")

            ensure_eden_settings(str(emulator_path))
            config_path = eden_dir / "user" / "config" / "qt-config.ini"
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("confirmStop = 2", text)
        self.assertIn("confirmStop\\default=false", text)
        self.assertNotIn("confirm_before_closing", text)

    def test_eden_fullscreen_and_first_start_set(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")

            ensure_eden_settings(str(emulator_path))
            config_path = eden_dir / "user" / "config" / "qt-config.ini"
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("fullscreen = true", text)
        self.assertIn("fullscreen\\default=false", text)
        self.assertIn("firstStart = false", text)
        self.assertIn("firstStart\\default=false", text)

    def test_eden_audio_volume_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")

            ensure_eden_settings(str(emulator_path))
            config_path = eden_dir / "user" / "config" / "qt-config.ini"
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Audio]", text)
        self.assertIn("volume = 40", text)
        self.assertIn("volume\\default=false", text)
        self.assertIn("muteWhenInBackground = true", text)
        self.assertIn("muteWhenInBackground\\default=false", text)

    def test_eden_renderer_scaling_filter_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")

            ensure_eden_settings(str(emulator_path))
            config_path = eden_dir / "user" / "config" / "qt-config.ini"
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[Renderer]", text)
        self.assertIn("scaling_filter = 6", text)
        self.assertIn("scaling_filter\\default=false", text)

    def test_eden_config_path_uses_user_config_subdir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")

            ensure_eden_settings(str(emulator_path))
            config_in_user_config = (eden_dir / "user" / "config" / "qt-config.ini").exists()
            config_at_root = (eden_dir / "qt-config.ini").exists()

        self.assertTrue(config_in_user_config)
        self.assertFalse(config_at_root)

    def test_eden_audio_volume_enforcement_overwrites_existing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")
            config_path = eden_dir / "user" / "config" / "qt-config.ini"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text("[Audio]\nvolume = 80\n", encoding="utf-8")

            ensure_eden_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("volume = 40", text)
        self.assertIn("volume\\default=false", text)
        self.assertNotIn("volume = 80", text)

    def test_xemu_creates_toml(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xemu_dir = Path(temp_dir) / "xemu"
            xemu_dir.mkdir()
            emulator_path = xemu_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            (xemu_dir / "xbox_hdd.qcow2").write_bytes(b"")
            with patch.dict(os.environ, {"APPDATA": str(Path(temp_dir) / "appdata")}, clear=False):
                result = ensure_xemu_settings(str(emulator_path))
            config_path = Path(str(result["config_path"]))
            config_exists = config_path.exists()
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertTrue(config_exists)
        self.assertIn("[misc]", text)
        self.assertIn("check_for_updates = false", text)
        self.assertIn("[display]", text)
        self.assertIn("vsync = true", text)
        self.assertIn("[general]", text)
        self.assertIn("show_welcome = false", text)

    def test_xemu_show_welcome_added_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[misc]\ncheck_for_updates = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[general]", text)
        self.assertIn("show_welcome = false", text)

    def test_xemu_preserves_user_vsync_setting(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xemu_dir = Path(temp_dir) / "xemu"
            xemu_dir.mkdir()
            emulator_path = xemu_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = xemu_dir / "xemu.toml"
            config_path.write_text("[display]\nvsync = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("vsync = false", text)
        self.assertNotIn("vsync = true", text)

    def test_xemu_config_written_to_emulator_dir_on_fresh_install(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "xemu"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "xemu.exe"
            exe_path.write_bytes(b"")
            fake_appdata = Path(temp_dir) / "fake_appdata"

            with patch.dict(os.environ, {"APPDATA": str(fake_appdata)}, clear=False):
                result = ensure_xemu_settings(str(exe_path))

            config_path = Path(str(result["config_path"]))
            config_exists = (emulator_dir / "xemu.toml").exists()

        self.assertEqual((emulator_dir / "xemu.toml").resolve(), config_path.resolve())
        self.assertFalse(str(config_path).startswith(str(fake_appdata)))
        self.assertTrue(config_exists)

    def test_xemu_fullscreen_added_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[misc]\ncheck_for_updates = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[display.window]", text)
        self.assertIn("fullscreen_on_startup = true", text)

    def test_xemu_fullscreen_not_overwritten_if_present(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[display.window]\nfullscreen_on_startup = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("fullscreen_on_startup = false", text)
        self.assertNotIn("fullscreen_on_startup = true", text)

    def test_xemu_surface_scale_added_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[misc]\ncheck_for_updates = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("surface_scale = 2", text)

    def test_xemu_volume_limit_added_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[misc]\ncheck_for_updates = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("volume_limit = 0.4", text)

    def test_xemu_port1_driver_added_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[misc]\ncheck_for_updates = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn('port1_driver = "usb-xbox-gamepad"', text)

    def test_xemu_sys_files_paths_added_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[misc]\ncheck_for_updates = false\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("bootrom_path =", text)
        self.assertIn("flashrom_path =", text)
        self.assertIn("hdd_path =", text)
        self.assertIn("eeprom_path =", text)
        self.assertIn(str(tmp_dir), text.replace("\\\\", "\\"))

    def test_xemu_sys_files_use_correct_bios_names(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")

            ensure_xemu_settings(str(emulator_path))
            text = (tmp_dir / "xemu.toml").read_text(encoding="utf-8")

        self.assertIn("mcpx_1.0.bin", text)
        self.assertIn("complex_4627.bin", text)
        self.assertNotIn("mcpx-1.1.bin", text)
        self.assertNotIn("xbox-5838.bin", text)

    def test_xemu_sys_files_eeprom_not_overwritten_if_present(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            tmp_dir = Path(temp_dir)
            emulator_path = tmp_dir / "xemu.exe"
            emulator_path.write_bytes(b"")
            config_path = tmp_dir / "xemu.toml"
            config_path.write_text("[sys.files]\neeprom_path = '/custom/path/eeprom.bin'\n", encoding="utf-8")

            ensure_xemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("eeprom_path = '/custom/path/eeprom.bin'", text)

    def test_xemu_base_path_candidates_includes_flatpak_path(self) -> None:
        candidates = xemu_base_path_candidates("", "", lambda s: [])
        self.assertIn(Path.home() / ".var" / "app" / "app.xemu.xemu" / "data" / "xemu" / "xemu", candidates)

    def test_ppsspp_deletes_installed_txt(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PPSSPP"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "PPSSPPWindows64.exe"
            exe_path.write_bytes(b"")
            installed_txt = emulator_dir / "installed.txt"
            installed_txt.write_text("marker", encoding="utf-8")

            result = ensure_ppsspp_settings(str(exe_path))
            installed_exists = installed_txt.exists()

        self.assertTrue(result["changed"])
        self.assertFalse(installed_exists)

    def test_ppsspp_writes_default_settings_without_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PPSSPP"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "PPSSPPWindows64.exe"
            exe_path.write_bytes(b"")

            result = ensure_ppsspp_settings(str(exe_path))
            ini_path = emulator_dir / "memstick" / "PSP" / "SYSTEM" / "PPSSPP.INI"
            text = ini_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("CheckForNewVersion = False", text)
        self.assertIn("SaveStateSlotCount = 3", text)
        self.assertIn("InternalResolution = 4", text)
        self.assertIn("TexScalingLevel = 4", text)
        self.assertIn("Smart2DTexFiltering = True", text)
        self.assertIn("HardwareTessellation = False", text)
        self.assertIn("GameVolume = 25", text)
        self.assertIn("ThemeName = Slate Forest", text)
        self.assertNotIn("[Achievements]", text)

    def test_ppsspp_no_change_for_empty_path(self) -> None:
        result = ensure_ppsspp_settings("")

        self.assertFalse(result["changed"])

    def test_ppsspp_writes_retroachievements_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PPSSPP"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "PPSSPPWindows64.exe"
            exe_path.write_bytes(b"")

            result = ensure_ppsspp_settings(
                str(exe_path),
                retroachievements_username="psp_user",
                retroachievements_token="psp_tok",
            )
            ini_path = emulator_dir / "memstick" / "PSP" / "SYSTEM" / "PPSSPP.INI"
            dat_path = emulator_dir / "memstick" / "PSP" / "SYSTEM" / "ppsspp_retroachievements.dat"
            text = ini_path.read_text(encoding="utf-8")
            dat_text = dat_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[Achievements]", text)
        self.assertIn("AchievementsEnable = True", text)
        self.assertNotIn("AchievementsEnableRAIntegration", text)
        self.assertIn("AchievementsUserName = psp_user", text)
        self.assertIn("AchievementsToken = psp_tok", text)
        self.assertIn("AchievementsChallengeMode = False", text)
        self.assertIn("AchievementsLeaderboardTrackerPos = 3", text)
        self.assertIn("AchievementsLeaderboardStartedOrFailedPos = 3", text)
        self.assertIn("AchievementsLeaderboardSubmittedPos = 3", text)
        self.assertIn("AchievementsProgressPos = 3", text)
        self.assertIn("AchievementsChallengePos = 3", text)
        self.assertIn("AchievementsUnlockedPos = 4", text)
        self.assertEqual(dat_text, "psp_tok")

    def test_ppsspp_skips_retroachievements_when_no_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PPSSPP"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "PPSSPPWindows64.exe"
            exe_path.write_bytes(b"")

            ensure_ppsspp_settings(str(exe_path))
            ini_path = emulator_dir / "memstick" / "PSP" / "SYSTEM" / "PPSSPP.INI"
            dat_path = emulator_dir / "memstick" / "PSP" / "SYSTEM" / "ppsspp_retroachievements.dat"
            ini_exists = ini_path.exists()
            dat_exists = dat_path.exists()
            text = ini_path.read_text(encoding="utf-8") if ini_exists else ""

        self.assertTrue(ini_exists)
        self.assertFalse(dat_exists)
        self.assertNotIn("[Achievements]", text)
        self.assertNotIn("AchievementsUserName", text)

    def test_redream_ensure_writes_fullscreen_and_volume(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            redream_dir = Path(temp_dir) / "Redream"
            redream_dir.mkdir()
            emulator_path = redream_dir / "redream.exe"
            emulator_path.write_bytes(b"")
            cfg_path = redream_dir / "redream.cfg"
            cfg_path.write_text("", encoding="utf-8")

            result = ensure_redream_settings(str(emulator_path))
            text = cfg_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("mode=fullscreen", text)
        self.assertIn("volume=40", text)

    def test_redream_ensure_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            redream_dir = Path(temp_dir) / "Redream"
            redream_dir.mkdir()
            emulator_path = redream_dir / "redream.exe"
            emulator_path.write_bytes(b"")
            cfg_path = redream_dir / "redream.cfg"
            cfg_path.write_text("mode=fullscreen\nvolume=40\n", encoding="utf-8")

            result = ensure_redream_settings(str(emulator_path))
            text = cfg_path.read_text(encoding="utf-8")

        self.assertFalse(result["changed"])
        self.assertEqual(text.count("mode=fullscreen"), 1)
        self.assertEqual(text.count("volume=40"), 1)

    def test_redream_ensure_updates_existing_values(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            redream_dir = Path(temp_dir) / "Redream"
            redream_dir.mkdir()
            emulator_path = redream_dir / "redream.exe"
            emulator_path.write_bytes(b"")
            cfg_path = redream_dir / "redream.cfg"
            cfg_path.write_text("mode=windowed\nvolume=100\n", encoding="utf-8")

            result = ensure_redream_settings(str(emulator_path))
            text = cfg_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("mode=fullscreen", text)
        self.assertIn("volume=40", text)
        self.assertNotIn("mode=windowed", text)
        self.assertNotIn("volume=100", text)

    def test_autoprofile_no_documents_or_appdata_tokens(self) -> None:
        profile_path = Path(__file__).resolve().parents[1] / "emulator-autoprofiles.json"
        with open(profile_path, encoding="utf-8") as f:
            profiles = json.load(f)

        blocked_tokens = ("%DOCUMENTS%", "%APPDATA%", "%LOCALAPPDATA%", "%USERPROFILE%")
        for profile in profiles:
            for key in ("save_directories", "state_directories", "screenshot_directories"):
                for directory in profile.get(key, []):
                    for token in blocked_tokens:
                        self.assertNotIn(token, directory)

    def test_cemu_creates_xml_when_absent(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            with patch.dict(
                os.environ,
                {
                    "APPDATA": str(Path(temp_dir) / "appdata"),
                    "LOCALAPPDATA": str(Path(temp_dir) / "localappdata"),
                },
                clear=False,
            ):
                result = ensure_cemu_settings(str(emulator_path))
            config_path = Path(str(result["config_path"]))
            config_exists = config_path.exists()
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertTrue(config_exists)
        self.assertEqual(cemu_dir / "portable" / "settings.xml", config_path)
        self.assertIn("<check_update>false</check_update>", text)
        self.assertIn("<use_discord_presence>false</use_discord_presence>", text)
        self.assertIn("<receive_untested_updates>false</receive_untested_updates>", text)
        self.assertIn("<gp_download>true</gp_download>", text)
        self.assertIn("<fullscreen>false</fullscreen>", text)
        self.assertIn("<window_maximized>true</window_maximized>", text)
        self.assertIn("<api>3</api>", text)

    def test_cemu_audio_block_created_when_absent(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")

            result = ensure_cemu_settings(str(emulator_path))
            config_path = cemu_dir / "portable" / "settings.xml"
            text = config_path.read_text(encoding="utf-8")
            root = ET.fromstring(config_path.read_text(encoding="utf-8"))
            audio = root.find("Audio")

        self.assertTrue(result["changed"])
        self.assertIsNotNone(audio)
        self.assertEqual("3", audio.findtext("api"))
        self.assertEqual("30", audio.findtext("TVVolume"))
        self.assertIn("<TVDevice>default</TVDevice>", text)

    def test_cemu_creates_portable_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")

            ensure_cemu_settings(str(emulator_path))
            portable_is_dir = (cemu_dir / "portable").is_dir()

        self.assertTrue(portable_is_dir)

    def test_cemu_settings_xml_written_to_portable_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")

            result = ensure_cemu_settings(str(emulator_path))

        self.assertEqual(cemu_dir / "portable" / "settings.xml", Path(str(result["config_path"])))

    def test_cemu_settings_path_candidates_windows_uses_appdata(self) -> None:
        with patch("sys.platform", "win32"):
            with patch.dict(
                os.environ,
                {"APPDATA": "/fake/appdata", "LOCALAPPDATA": "/fake/localappdata"},
                clear=False,
            ):
                candidates = cemu_settings_path_candidates("")

        self.assertIn(Path("/fake/appdata").expanduser() / "Cemu" / "settings.xml", candidates)
        self.assertIn(Path("/fake/localappdata").expanduser() / "Cemu" / "settings.xml", candidates)

    def test_cemu_settings_path_candidates_linux_uses_xdg_and_flatpak(self) -> None:
        with patch("sys.platform", "linux"):
            with patch.dict(os.environ, {"APPDATA": "/some/windows/path"}, clear=False):
                candidates = cemu_settings_path_candidates("")

        self.assertIn(xdg_config_home() / "Cemu" / "settings.xml", candidates)
        self.assertIn(
            Path.home() / ".var" / "app" / "info.cemu.Cemu" / "config" / "Cemu" / "settings.xml",
            candidates,
        )
        self.assertNotIn(Path("/some/windows/path").expanduser() / "Cemu" / "settings.xml", candidates)

    def test_cemu_settings_path_candidates_honors_xdg_config_home_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xdg_config_home_override = Path(temp_dir) / "xdg-config"
            with patch("sys.platform", "linux"):
                with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(xdg_config_home_override)}, clear=False):
                    candidates = cemu_settings_path_candidates("")

        self.assertIn(xdg_config_home_override / "Cemu" / "settings.xml", candidates)

    def test_cemu_overwrites_check_update_true(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<check_update>true</check_update>",
                        "<use_discord_presence>false</use_discord_presence>",
                        "<receive_untested_updates>false</receive_untested_updates>",
                        "<gp_download>true</gp_download>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("<check_update>false</check_update>", text)

    def test_cemu_enforces_use_discord_presence_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<use_discord_presence>true</use_discord_presence>",
                        "<check_update>false</check_update>",
                        "<receive_untested_updates>false</receive_untested_updates>",
                        "<gp_download>true</gp_download>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("<use_discord_presence>false</use_discord_presence>", text)

    def test_cemu_enforces_receive_untested_updates_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<use_discord_presence>false</use_discord_presence>",
                        "<check_update>false</check_update>",
                        "<receive_untested_updates>true</receive_untested_updates>",
                        "<gp_download>true</gp_download>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("<receive_untested_updates>false</receive_untested_updates>", text)

    def test_cemu_enforces_gp_download_true(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<use_discord_presence>false</use_discord_presence>",
                        "<check_update>false</check_update>",
                        "<receive_untested_updates>false</receive_untested_updates>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("<gp_download>true</gp_download>", text)

    def test_cemu_enforces_fullscreen_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<use_discord_presence>false</use_discord_presence>",
                        "<check_update>false</check_update>",
                        "<receive_untested_updates>false</receive_untested_updates>",
                        "<gp_download>true</gp_download>",
                        "<fullscreen>true</fullscreen>",
                        "<window_maximized>true</window_maximized>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("<fullscreen>false</fullscreen>", text)

    def test_cemu_enforces_window_maximized_true(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<use_discord_presence>false</use_discord_presence>",
                        "<check_update>false</check_update>",
                        "<receive_untested_updates>false</receive_untested_updates>",
                        "<gp_download>true</gp_download>",
                        "<fullscreen>false</fullscreen>",
                        "<window_maximized>false</window_maximized>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("<window_maximized>true</window_maximized>", text)

    def test_cemu_preserves_unmanaged_settings(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<mlc_path>/some/path</mlc_path>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            ensure_cemu_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("<mlc_path>/some/path</mlc_path>", text)

    def test_cemu_no_change_when_all_already_correct(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "portable" / "settings.xml"
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<use_discord_presence>false</use_discord_presence>",
                        "<check_update>false</check_update>",
                        "<receive_untested_updates>false</receive_untested_updates>",
                        "<gp_download>true</gp_download>",
                        "<fullscreen>false</fullscreen>",
                        "<window_maximized>true</window_maximized>",
                        "<Audio><api>3</api><TVVolume>100</TVVolume><TVDevice></TVDevice></Audio>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))

        self.assertFalse(result["changed"])

    def test_cemu_controller_config_creates_controller0_xml(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            result = ensure_cemu_controller_config(temp_dir)
            profile_path = Path(temp_dir) / "portable" / "controllerProfiles" / "controller0.xml"
            profile_exists = profile_path.exists()

        self.assertTrue(profile_exists)
        self.assertTrue(result["changed"])

    def test_cemu_controller_config_skips_if_already_exists(self) -> None:
        custom_content = "<custom>true</custom>\n"
        with tempfile.TemporaryDirectory() as temp_dir:
            profile_path = Path(temp_dir) / "portable" / "controllerProfiles" / "controller0.xml"
            profile_path.parent.mkdir(parents=True, exist_ok=True)
            profile_path.write_text(custom_content, encoding="utf-8")

            result = ensure_cemu_controller_config(temp_dir)
            written = profile_path.read_text(encoding="utf-8")

        self.assertEqual(custom_content, written)
        self.assertFalse(result["changed"])

    def test_cemu_controller_config_xml_contains_xinput_api(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            with patch("grid_launcher.emulator.cemu.sys.platform", "win32"):
                ensure_cemu_controller_config(temp_dir)
            profile_path = Path(temp_dir) / "portable" / "controllerProfiles" / "controller0.xml"
            text = profile_path.read_text(encoding="utf-8")

        self.assertIn("<api>XInput</api>", text)
        self.assertIn("<type>Wii U Pro Controller</type>", text)

    def test_cemu_controller_config_linux_writes_sdl_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            with patch("grid_launcher.emulator.cemu.sys.platform", "linux"):
                ensure_cemu_controller_config(temp_dir)
            profile_path = Path(temp_dir) / "portable" / "controllerProfiles" / "controller0.xml"
            text = profile_path.read_text(encoding="utf-8")

        self.assertIn("<api>SDLController</api>", text)
        self.assertNotIn("<api>XInput</api>", text)
        self.assertIn("<type>Wii U Pro Controller</type>", text)


class Rpcs3AutoConfigTests(unittest.TestCase):
    def test_rpcs3_data_root_returns_portable_when_exists(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            portable_dir = Path(tmp) / "portable"
            portable_dir.mkdir()

            result = rpcs3_data_root(str(exe))

        self.assertEqual(portable_dir.resolve(), result)

    def test_rpcs3_data_root_returns_emulator_dir_when_no_portable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")

            result = rpcs3_data_root(str(exe))

        self.assertEqual(Path(tmp).resolve(), result)

    def test_rpcs3_creates_portable_directory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            ensure_rpcs3_settings(str(exe))
            self.assertTrue((Path(tmp) / "portable").exists())

    def test_rpcs3_writes_config_yml_fullscreen(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            ensure_rpcs3_settings(str(exe))
            text = (Path(tmp) / "portable" / "config" / "config.yml").read_text(encoding="utf-8")
        self.assertIn("Start games in fullscreen mode: true", text)

    def test_rpcs3_writes_config_yml_volume(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            ensure_rpcs3_settings(str(exe))
            text = (Path(tmp) / "portable" / "config" / "config.yml").read_text(encoding="utf-8")
        self.assertIn("Master Volume: 40", text)

    def test_rpcs3_preserves_existing_yaml_value(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            config_dir = Path(tmp) / "portable" / "config"
            config_dir.mkdir(parents=True)
            (config_dir / "config.yml").write_text(
                "Miscellaneous:\n  Start games in fullscreen mode: false\n",
                encoding="utf-8",
            )
            ensure_rpcs3_settings(str(exe))
            text = (config_dir / "config.yml").read_text(encoding="utf-8")
        self.assertIn("Start games in fullscreen mode: false", text)
        self.assertNotIn("Start games in fullscreen mode: true", text)

    def test_rpcs3_writes_gui_settings_wizard_suppression(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            ensure_rpcs3_settings(str(exe))
            text = (Path(tmp) / "portable" / "GuiConfigs" / "GuiSettings.ini").read_text(encoding="utf-8")
        self.assertIn("[main_window]", text)
        self.assertIn("infoBoxEnabledWelcome = false", text)

    def test_rpcs3_gui_settings_contains_only_main_window_section(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            ensure_rpcs3_settings(str(exe))
            text = (Path(tmp) / "portable" / "GuiConfigs" / "GuiSettings.ini").read_text(encoding="utf-8")
        self.assertIn("[main_window]", text)
        self.assertIn("infoBoxEnabledWelcome = false", text)
        self.assertIn("confirmationBoxExitGame = false", text)
        self.assertIn("confirmationBoxBootGame = false", text)
        self.assertIn("infoBoxEnabledInstallPUP = false", text)
        self.assertNotIn("[Meta]", text)
        self.assertNotIn("useRichPresence", text)
        self.assertNotIn("checkUpdateStart", text)

    def test_rpcs3_writes_current_settings_check_update_and_rich_presence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            ensure_rpcs3_settings(str(exe))
            text = (Path(tmp) / "portable" / "GuiConfigs" / "CurrentSettings.ini").read_text(encoding="utf-8")
        self.assertIn("[Meta]", text)
        self.assertIn("checkUpdateStart=false", text)
        self.assertIn("useRichPresence=false", text)
        self.assertIn("[main_window]", text)
        self.assertIn("infoBoxEnabledWelcome=false", text)
        self.assertIn("confirmationBoxExitGame=false", text)
        self.assertIn("confirmationBoxBootGame=false", text)
        self.assertIn("infoBoxEnabledInstallPUP=false", text)

    def test_rpcs3_always_overwrites_gui_ini_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            gui_dir = Path(tmp) / "portable" / "GuiConfigs"
            gui_dir.mkdir(parents=True)
            (gui_dir / "GuiSettings.ini").write_text(
                "[main_window]\ninfoBoxEnabledWelcome=true\n",
                encoding="utf-8",
            )
            ensure_rpcs3_settings(str(exe))
            text = (gui_dir / "GuiSettings.ini").read_text(encoding="utf-8")
        self.assertIn("infoBoxEnabledWelcome = false", text)
        self.assertNotIn("infoBoxEnabledWelcome=true", text)

    def test_rpcs3_always_overwrites_current_settings_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            gui_dir = Path(tmp) / "portable" / "GuiConfigs"
            gui_dir.mkdir(parents=True)
            (gui_dir / "CurrentSettings.ini").write_text(
                "[Meta]\ncheckUpdateStart=true\nuseRichPresence=true\n[main_window]\ninfoBoxEnabledWelcome=true\n",
                encoding="utf-8",
            )
            ensure_rpcs3_settings(str(exe))
            text = (gui_dir / "CurrentSettings.ini").read_text(encoding="utf-8")
        self.assertIn("checkUpdateStart=false", text)
        self.assertIn("useRichPresence=false", text)
        self.assertIn("infoBoxEnabledWelcome=false", text)
        self.assertNotIn("checkUpdateStart=true", text)
        self.assertNotIn("useRichPresence=true", text)
        self.assertNotIn("infoBoxEnabledWelcome=true", text)

    def test_rpcs3_returns_changed_true_on_first_run(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            result = ensure_rpcs3_settings(str(exe))
        self.assertTrue(result["changed"])

    def test_rpcs3_returns_changed_false_on_second_run(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            ensure_rpcs3_settings(str(exe))
            result = ensure_rpcs3_settings(str(exe))
        self.assertFalse(result["changed"])

    def test_rpcs3_data_root_candidates_honors_xdg_config_home_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xdg_config_home_override = Path(temp_dir) / "xdg-config"
            with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(xdg_config_home_override)}, clear=False):
                os.environ.pop("RPCS3_CONFIG_DIR", None)
                candidates = rpcs3_data_root_candidates("")

        self.assertIn(xdg_config_home_override / "rpcs3", candidates)

    def test_rpcs3_data_root_candidates_includes_flatpak_path(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("RPCS3_CONFIG_DIR", None)
            candidates = rpcs3_data_root_candidates("")

        self.assertIn(Path.home() / ".var" / "app" / "net.rpcs3.RPCS3" / "config" / "rpcs3", candidates)

    def test_rpcs3_data_root_candidates_order_is_unchanged_aside_from_flatpak_addition(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "rpcs3.exe"
            exe.write_bytes(b"")
            portable_dir = Path(tmp) / "portable"
            portable_dir.mkdir()

            with patch.dict(os.environ, {"RPCS3_CONFIG_DIR": "/fake/rpcs3-config"}, clear=False):
                candidates = rpcs3_data_root_candidates(str(exe))

        self.assertEqual(
            [
                portable_dir.resolve(),
                Path("/fake/rpcs3-config").expanduser().resolve(),
                Path(tmp).resolve(),
                xdg_config_home() / "rpcs3",
                Path.home() / ".var" / "app" / "net.rpcs3.RPCS3" / "config" / "rpcs3",
                Path.home() / "Library" / "Application Support" / "rpcs3",
            ],
            candidates,
        )

    def test_pico8_user_root_candidates_honors_xdg_data_home_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xdg_data_home = Path(temp_dir) / "xdg-data"
            with patch.dict(os.environ, {"XDG_DATA_HOME": str(xdg_data_home)}, clear=False):
                with patch("sys.platform", "linux"):
                    candidates = pico8_user_root_candidates("", "", lambda s: [])

        self.assertIn((xdg_data_home / "pico-8").resolve(), candidates)

    def test_pico8_user_root_candidates_keeps_dotfile_dir_as_fallback_when_xdg_unset(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("XDG_DATA_HOME", None)
            with patch("sys.platform", "linux"):
                candidates = pico8_user_root_candidates("", "", lambda s: [])

        self.assertIn((Path.home() / ".lexaloffle" / "pico-8").resolve(), candidates)


class EmulatorEnsureDispatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_ensure_dispatch_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load grid-launcher.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    def test_ensure_emulator_sync_settings_calls_ppsspp_settings(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {
                    "retroachievements_username": "psp_user",
                    "retroachievements_token": "psp_token",
                }

            def _is_retroarch_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_duckstation_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_xemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_pcsx2_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[tuple[str, str]]:
                return []

            def _is_dolphin_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_azahar_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_eden_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_rpcs3_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_ppsspp_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return True

            def _is_cemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_redream_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

        window = _WindowStub()

        with patch("grid_launcher.ui.mixins.emulator_ui_mixin.ensure_ppsspp_settings") as ensure_ppsspp:
            module.MainWindow._ensure_emulator_sync_settings(window, "PPSSPP", "C:/Emulators/PPSSPPWindows64.exe")

        ensure_ppsspp.assert_called_once_with(
            "C:/Emulators/PPSSPPWindows64.exe",
            retroachievements_username="psp_user",
            retroachievements_token="psp_token",
        )

    def test_on_ra_login_finished_syncs_all_emulator_settings(self) -> None:
        module = type(self).module

        calls: list[tuple[str, str]] = []

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {
                    "emulators": [
                        {"name": "PPSSPP", "path": "C:/Emulators/PPSSPPWindows64.exe"},
                        {"name": "RetroArch", "path": "C:/Emulators/retroarch.exe"},
                    ]
                }
                self.ra_login_button = None
                self.ra_username_input = None
                self.ra_password_input = None
                self.ra_login_status_label = None

            def _save_ra_token(self, token: str) -> None:
                pass

            def _emulators(self) -> list[dict]:
                return self.config.get("emulators", [])

            def _ensure_emulator_sync_settings(self, emulator_name: str, emulator_path: str) -> None:
                calls.append((emulator_name, emulator_path))

            def _show_toast(self, message: str) -> None:
                pass

        window = _WindowStub()
        module.MainWindow._on_ra_login_finished(
            window,
            {"username": "psp_user", "token": "new_token", "error": ""},
        )

        self.assertEqual(window.config.get("retroachievements_username"), "psp_user")
        self.assertEqual(window.config.get("retroachievements_token"), "new_token")
        self.assertIn(("PPSSPP", "C:/Emulators/PPSSPPWindows64.exe"), calls)
        self.assertIn(("RetroArch", "C:/Emulators/retroarch.exe"), calls)

    def test_on_ra_login_finished_does_not_sync_on_error(self) -> None:
        module = type(self).module

        sync_called = []

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {
                    "emulators": [
                        {"name": "PPSSPP", "path": "C:/Emulators/PPSSPPWindows64.exe"},
                    ]
                }
                self.ra_login_button = None
                self.ra_login_status_label = None

            def _ensure_emulator_sync_settings(self, emulator_name: str, emulator_path: str) -> None:
                sync_called.append(emulator_name)

            def _show_toast(self, message: str) -> None:
                pass

        window = _WindowStub()
        module.MainWindow._on_ra_login_finished(
            window,
            {"username": "", "token": "", "error": "Invalid credentials"},
        )

        self.assertEqual(sync_called, [])


class TestRpcs3FirmwareBackgroundDownload(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_rpcs3_firmware_background_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load grid-launcher.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    def test_trigger_background_download_skipped_if_pup_exists(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self._emulator_refresh_requested = type("_SignalStub", (), {"emit": lambda _self: None})()
                self._toast_requested = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_progress = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_done = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_entry_id: str | None = None
                self.active_download_count = 0
                self.active_download_bytes = 0
                self.active_download_total = 0
                self.active_download_speed_bps = 0.0

            def _create_download_entry(self, game: dict, status: str) -> str:
                return "stub-entry-id"

            def _update_download_status_ui(self) -> None:
                pass

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[Path]:
                del emulator_entry
                return [Path("C:/firmware")]

        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "rpcs3.exe"
            emulator_path.write_bytes(b"")
            (Path(temp_dir) / "PS3UPDAT.PUP").write_bytes(b"fake")
            window = _WindowStub()

            with patch.object(module, "download_ps3_firmware_direct") as download_direct:
                module.MainWindow._trigger_rpcs3_firmware_download_background(
                    window,
                    {"name": "RPCS3", "path": str(emulator_path)},
                    str(emulator_path),
                )

        download_direct.assert_not_called()

    def test_trigger_background_download_starts_thread_without_server_platform_id(self) -> None:
        # Platform ID is no longer required; firmware is fetched directly from Sony.
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self._emulator_refresh_requested = type("_SignalStub", (), {"emit": lambda _self: None})()
                self._toast_requested = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_progress = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_done = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_entry_id: str | None = None
                self.active_download_count = 0
                self.active_download_bytes = 0
                self.active_download_total = 0
                self.active_download_speed_bps = 0.0

            def _create_download_entry(self, game: dict, status: str) -> str:
                return "stub-entry-id"

            def _update_download_status_ui(self) -> None:
                pass

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[Path]:
                del emulator_entry
                return [Path("C:/firmware")]

        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "rpcs3.exe"
            emulator_path.write_bytes(b"")
            window = _WindowStub()

            thread_started = []

            class _ThreadStub:
                def __init__(self, *args, **kwargs):
                    pass

                def start(self):
                    thread_started.append(True)

            with patch("threading.Thread", _ThreadStub):
                module.MainWindow._trigger_rpcs3_firmware_download_background(
                    window,
                    {"name": "RPCS3", "path": str(emulator_path)},
                    str(emulator_path),
                )

        self.assertEqual(len(thread_started), 1)

    def test_trigger_background_download_skipped_if_no_firmware_dirs(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.server_platform_ids = {"PlayStation 3": 5}
                self._api_get = object()
                self._api_get_bytes = object()
                self._emulator_refresh_requested = type("_SignalStub", (), {"emit": lambda _self: None})()
                self._toast_requested = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_progress = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_done = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_entry_id: str | None = None
                self.active_download_count = 0
                self.active_download_bytes = 0
                self.active_download_total = 0
                self.active_download_speed_bps = 0.0

            def _create_download_entry(self, game: dict, status: str) -> str:
                return "stub-entry-id"

            def _update_download_status_ui(self) -> None:
                pass

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[Path]:
                del emulator_entry
                return []

        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "rpcs3.exe"
            emulator_path.write_bytes(b"")
            window = _WindowStub()

            with patch("threading.Thread") as thread_ctor:
                module.MainWindow._trigger_rpcs3_firmware_download_background(
                    window,
                    {"name": "RPCS3", "path": str(emulator_path)},
                    str(emulator_path),
                )

        thread_ctor.assert_not_called()

    def test_trigger_background_download_calls_install_and_refreshes(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self._emulator_refresh_requested = type("_SignalStub", (), {"emit": lambda _self: None})()
                self._toast_requested = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_progress = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_done = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_entry_id: str | None = None
                self.active_download_count = 0
                self.active_download_bytes = 0
                self.active_download_total = 0
                self.active_download_speed_bps = 0.0

            def _create_download_entry(self, game: dict, status: str) -> str:
                return "stub-entry-id"

            def _update_download_status_ui(self) -> None:
                pass

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[Path]:
                del emulator_entry
                return [Path("C:/firmware")]

            def _refresh_emulator_views(self) -> None:
                return None

        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "rpcs3.exe"
            emulator_path.write_bytes(b"")
            window = _WindowStub()
            expected_firmware_dirs = [Path("C:/firmware")]

            def _thread_side_effect(*args, **kwargs):
                target = kwargs.get("target")
                self.assertTrue(callable(target))
                target()

                class _ThreadStub:
                    def start(self) -> None:
                        return None

                return _ThreadStub()

            with (
                patch("grid_launcher.ui.mixins.emulator_ui_mixin.download_ps3_firmware_direct", return_value=[]) as download_direct,
                patch("grid_launcher.ui.mixins.emulator_ui_mixin.install_platform_firmware") as install_firmware,
                patch.object(window._emulator_refresh_requested, "emit") as emit_signal,
                patch("threading.Thread", side_effect=_thread_side_effect),
            ):
                module.MainWindow._trigger_rpcs3_firmware_download_background(
                    window,
                    {"name": "RPCS3", "path": str(emulator_path)},
                    str(emulator_path),
                )

        download_direct.assert_called_once_with(
            expected_firmware_dirs,
            skip_existing=True,
            progress_callback=ANY,
        )
        install_firmware.assert_not_called()
        emit_signal.assert_called_once_with()

    def test_trigger_background_download_falls_back_to_romm_on_sony_failure(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.server_platform_ids = {"PlayStation 3": 5}
                self._api_get = object()
                self._api_get_bytes = object()
                self._emulator_refresh_requested = type("_SignalStub", (), {"emit": lambda _self: None})()
                self._toast_requested = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_progress = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_done = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_entry_id: str | None = None
                self.active_download_count = 0
                self.active_download_bytes = 0
                self.active_download_total = 0
                self.active_download_speed_bps = 0.0

            def _create_download_entry(self, game: dict, status: str) -> str:
                return "stub-entry-id"

            def _update_download_status_ui(self) -> None:
                pass

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[Path]:
                del emulator_entry
                return [Path("C:/firmware")]

        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "rpcs3.exe"
            emulator_path.write_bytes(b"")
            window = _WindowStub()
            expected_firmware_dirs = [Path("C:/firmware")]

            def _thread_side_effect(*args, **kwargs):
                target = kwargs.get("target")
                self.assertTrue(callable(target))
                target()

                class _ThreadStub:
                    def start(self) -> None:
                        return None

                return _ThreadStub()

            with (
                patch(
                    "grid_launcher.ui.mixins.emulator_ui_mixin.download_ps3_firmware_direct", return_value=["network error"]
                ) as download_direct,
                patch("grid_launcher.ui.mixins.emulator_ui_mixin.install_platform_firmware") as install_firmware,
                patch.object(window._emulator_refresh_requested, "emit") as emit_signal,
                patch("threading.Thread", side_effect=_thread_side_effect),
            ):
                module.MainWindow._trigger_rpcs3_firmware_download_background(
                    window,
                    {"name": "RPCS3", "path": str(emulator_path)},
                    str(emulator_path),
                )

        download_direct.assert_called_once_with(expected_firmware_dirs, skip_existing=True, progress_callback=ANY)
        install_firmware.assert_called_once_with(
            window._api_get,
            window._api_get_bytes,
            5,
            expected_firmware_dirs,
            skip_existing=True,
        )
        emit_signal.assert_called_once_with()

    def test_trigger_background_download_no_romm_fallback_without_platform_id(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.server_platform_ids = {}
                self._api_get = object()
                self._api_get_bytes = object()
                self._emulator_refresh_requested = type("_SignalStub", (), {"emit": lambda _self: None})()
                self._toast_requested = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_progress = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_done = type("_SignalStub", (), {"emit": lambda _self, *a: None})()
                self._firmware_download_entry_id: str | None = None
                self.active_download_count = 0
                self.active_download_bytes = 0
                self.active_download_total = 0
                self.active_download_speed_bps = 0.0

            def _create_download_entry(self, game: dict, status: str) -> str:
                return "stub-entry-id"

            def _update_download_status_ui(self) -> None:
                pass

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[Path]:
                del emulator_entry
                return [Path("C:/firmware")]

        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_path = Path(temp_dir) / "rpcs3.exe"
            emulator_path.write_bytes(b"")
            window = _WindowStub()

            def _thread_side_effect(*args, **kwargs):
                target = kwargs.get("target")
                self.assertTrue(callable(target))
                target()

                class _ThreadStub:
                    def start(self) -> None:
                        return None

                return _ThreadStub()

            with (
                patch("grid_launcher.ui.mixins.emulator_ui_mixin.download_ps3_firmware_direct", return_value=["network error"]),
                patch("grid_launcher.ui.mixins.emulator_ui_mixin.install_platform_firmware") as install_firmware,
                patch("threading.Thread", side_effect=_thread_side_effect),
            ):
                module.MainWindow._trigger_rpcs3_firmware_download_background(
                    window,
                    {"name": "RPCS3", "path": str(emulator_path)},
                    str(emulator_path),
                )

        install_firmware.assert_not_called()


class XemuMissingBiosFilesTests(unittest.TestCase):

    def test_all_present_returns_empty(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_dir = Path(tmp)
            for name in ("mcpx_1.0.bin", "complex_4627.bin", "xbox_hdd.qcow2"):
                (tmp_dir / name).write_bytes(b"")
            result = xemu_missing_bios_files(str(tmp_dir / "xemu.exe"))
            self.assertEqual(result, [])

    def test_all_absent_returns_all_required(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_dir = Path(tmp)
            result = xemu_missing_bios_files(str(tmp_dir / "xemu.exe"))
            self.assertIn("mcpx_1.0.bin", result)
            self.assertIn("complex_4627.bin", result)
            self.assertIn("xbox_hdd.qcow2", result)

    def test_partial_missing_returns_missing_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_dir = Path(tmp)
            (tmp_dir / "xbox_hdd.qcow2").write_bytes(b"")
            result = xemu_missing_bios_files(str(tmp_dir / "xemu.exe"))
            self.assertIn("mcpx_1.0.bin", result)
            self.assertIn("complex_4627.bin", result)
            self.assertNotIn("xbox_hdd.qcow2", result)

    def test_eeprom_absence_not_included(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_dir = Path(tmp)
            # eeprom.bin absent but it should not appear in missing list
            result = xemu_missing_bios_files(str(tmp_dir / "xemu.exe"))
            self.assertNotIn("eeprom.bin", result)


class Rpcs3VfsSettingsTests(unittest.TestCase):
    def _make_exe(self, tmp: str) -> Path:
        exe = Path(tmp) / "rpcs3.exe"
        exe.write_bytes(b"")
        return exe

    def test_ensure_rpcs3_vfs_settings_writes_vfs_yml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            library = Path(tmp) / "PS3 Library"
            from grid_launcher.emulator.rpcs3 import ensure_rpcs3_vfs_settings
            result = ensure_rpcs3_vfs_settings(str(exe), str(library))

            self.assertTrue(result["changed"])
            vfs_path = Path(str(result["vfs_path"]))
            self.assertTrue(vfs_path.exists())
            content = vfs_path.read_text(encoding="utf-8")
            self.assertIn("/dev_hdd0/", content)
            self.assertIn("/games/", content)
            self.assertIn(".vfs", content)

    def test_ensure_rpcs3_vfs_settings_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            library = Path(tmp) / "PS3 Library"
            from grid_launcher.emulator.rpcs3 import ensure_rpcs3_vfs_settings
            result1 = ensure_rpcs3_vfs_settings(str(exe), str(library))
            result2 = ensure_rpcs3_vfs_settings(str(exe), str(library))

            self.assertTrue(result1["changed"])
            self.assertFalse(result2["changed"])
            # Key should appear exactly once (not duplicated by second call)
            content = Path(str(result1["vfs_path"])).read_text(encoding="utf-8")
            self.assertEqual(content.count('"/dev_hdd0/":'), 1)
            self.assertEqual(content.count('"/games/":'), 1)

    def test_ensure_rpcs3_vfs_settings_does_not_overwrite_existing_dev_hdd0(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            library = Path(tmp) / "PS3 Library"
            # Write a pre-existing vfs.yml with a custom /dev_hdd0/ path
            config_dir = Path(tmp) / "portable" / "config"
            config_dir.mkdir(parents=True)
            vfs_path = config_dir / "vfs.yml"
            vfs_path.write_text(
                '"$(EmulatorDir)": ""\n"/dev_hdd0/": "D:/MyCustomPath/"\n',
                encoding="utf-8",
            )

            from grid_launcher.emulator.rpcs3 import ensure_rpcs3_vfs_settings
            result = ensure_rpcs3_vfs_settings(str(exe), str(library))

            self.assertTrue(result["changed"])
            content = vfs_path.read_text(encoding="utf-8")
            self.assertIn("D:/MyCustomPath/", content)
            self.assertIn('"/games/":', content)

    def test_ensure_rpcs3_vfs_settings_does_not_overwrite_existing_games(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            library = Path(tmp) / "PS3 Library"
            config_dir = Path(tmp) / "portable" / "config"
            config_dir.mkdir(parents=True)
            vfs_path = config_dir / "vfs.yml"
            vfs_path.write_text(
                '"$(EmulatorDir)": ""\n"/dev_hdd0/": "D:/MyCustomPath/"\n"/games/": "E:/DiscGames/"\n',
                encoding="utf-8",
            )

            from grid_launcher.emulator.rpcs3 import ensure_rpcs3_vfs_settings
            result = ensure_rpcs3_vfs_settings(str(exe), str(library))

            self.assertFalse(result["changed"])
            content = vfs_path.read_text(encoding="utf-8")
            self.assertIn('"/games/": "E:/DiscGames/"', content)

    def test_ensure_rpcs3_vfs_settings_returns_no_change_for_missing_exe(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            library = Path(tmp) / "PS3 Library"
            from grid_launcher.emulator.rpcs3 import ensure_rpcs3_vfs_settings
            result = ensure_rpcs3_vfs_settings(str(Path(tmp) / "nonexistent.exe"), str(library))

        self.assertFalse(result["changed"])
        self.assertIsNone(result["vfs_path"])

    def test_ps3_vfs_dev_hdd0_path_reads_from_existing_vfs_yml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            custom_path = Path(tmp) / "custom_hdd0"
            config_dir = Path(tmp) / "portable" / "config"
            config_dir.mkdir(parents=True)
            vfs_path = config_dir / "vfs.yml"
            vfs_path.write_text(f'"/dev_hdd0/": "{custom_path.as_posix()}/"\n', encoding="utf-8")

            from grid_launcher.emulator.rpcs3 import ps3_vfs_dev_hdd0_path
            result = ps3_vfs_dev_hdd0_path(str(exe), "")

        self.assertIsNotNone(result)
        self.assertEqual(result, custom_path)

    def test_ps3_vfs_dev_hdd0_path_falls_back_to_library_default(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            library = Path(tmp) / "PS3 Library"

            from grid_launcher.emulator.rpcs3 import ps3_vfs_dev_hdd0_path
            result = ps3_vfs_dev_hdd0_path(str(exe), str(library))

        self.assertIsNotNone(result)
        expected = library.resolve() / ".vfs" / "dev_hdd0"
        self.assertEqual(result, expected)

    def test_ps3_vfs_dev_hdd0_path_returns_none_with_no_vfs_and_no_library(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            from grid_launcher.emulator.rpcs3 import ps3_vfs_dev_hdd0_path
            result = ps3_vfs_dev_hdd0_path(str(exe), "")

        self.assertIsNone(result)

    def test_ps3_vfs_games_path_reads_from_existing_vfs_yml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            custom_path = Path(tmp) / "custom_games"
            config_dir = Path(tmp) / "portable" / "config"
            config_dir.mkdir(parents=True)
            vfs_path = config_dir / "vfs.yml"
            vfs_path.write_text(f'"/games/": "{custom_path.as_posix()}/"\n', encoding="utf-8")

            result = ps3_vfs_games_path(str(exe), "")

        self.assertIsNotNone(result)
        self.assertEqual(result, custom_path)

    def test_ps3_vfs_games_path_falls_back_to_library_default(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            library = Path(tmp) / "PS3 Library"

            result = ps3_vfs_games_path(str(exe), str(library))

        self.assertIsNotNone(result)
        expected = library.resolve() / ".vfs" / "games"
        self.assertEqual(result, expected)

    def test_ps3_vfs_games_path_returns_none_with_no_vfs_and_no_library(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            exe = self._make_exe(tmp)
            result = ps3_vfs_games_path(str(exe), "")

        self.assertIsNone(result)


class SourceVersionCheckHandlerTests(unittest.TestCase):
    class _StubWindow(EmulatorUIMixin):
        def __init__(self) -> None:
            self._do_start_source_emulator_update_at_index = MagicMock()
            self._source_check_emulator_name = "TestEmulator"

    @patch("grid_launcher.ui.mixins.emulator_ui_mixin.QMessageBox")
    def test_error_shows_warning(self, mock_message_box) -> None:
        stub = self._StubWindow()

        stub._on_source_version_check_finished("", "", "timeout", 0)

        mock_message_box.warning.assert_called_once()

    @patch("grid_launcher.ui.mixins.emulator_ui_mixin.QMessageBox")
    def test_version_match_shows_info_no_install(self, mock_message_box) -> None:
        stub = self._StubWindow()

        stub._on_source_version_check_finished("v1.2.3", "v1.2.3", "", 0)

        mock_message_box.information.assert_called_once()
        self.assertEqual(mock_message_box.information.call_args[0][1], "No Updates Available")
        stub._do_start_source_emulator_update_at_index.assert_not_called()

    @patch("grid_launcher.ui.mixins.emulator_ui_mixin.QMessageBox")
    def test_version_mismatch_yes_calls_install(self, mock_message_box) -> None:
        stub = self._StubWindow()
        mock_message_box.StandardButton.Yes = 1
        mock_message_box.StandardButton.No = 2
        mock_message_box.question.return_value = 1

        stub._on_source_version_check_finished("v1.2.2", "v1.2.3", "", 0)

        mock_message_box.question.assert_called_once()
        stub._do_start_source_emulator_update_at_index.assert_called_once_with(0)

    @patch("grid_launcher.ui.mixins.emulator_ui_mixin.QMessageBox")
    def test_version_mismatch_no_skips_install(self, mock_message_box) -> None:
        stub = self._StubWindow()
        mock_message_box.StandardButton.Yes = 1
        mock_message_box.StandardButton.No = 2
        mock_message_box.question.return_value = 2

        stub._on_source_version_check_finished("v1.2.2", "v1.2.3", "", 0)

        mock_message_box.question.assert_called_once()
        stub._do_start_source_emulator_update_at_index.assert_not_called()

    @patch("grid_launcher.ui.mixins.emulator_ui_mixin.QMessageBox")
    def test_latest_installed_shows_update_dialog(self, mock_message_box) -> None:
        stub = self._StubWindow()
        mock_message_box.StandardButton.Yes = 1
        mock_message_box.StandardButton.No = 2
        mock_message_box.question.return_value = 2

        stub._on_source_version_check_finished("latest", "v1.2.3", "", 0)

        mock_message_box.question.assert_called_once()
        mock_message_box.information.assert_not_called()

    @patch("grid_launcher.ui.mixins.emulator_ui_mixin.QMessageBox")
    def test_direct_provider_shows_update_dialog(self, mock_message_box) -> None:
        stub = self._StubWindow()
        mock_message_box.StandardButton.Yes = 1
        mock_message_box.StandardButton.No = 2
        mock_message_box.question.return_value = 2

        stub._on_source_version_check_finished("v1.0.0", "direct", "", 0)

        mock_message_box.question.assert_called_once()
        mock_message_box.information.assert_not_called()


class EmulatorSharedDataPathTests(unittest.TestCase):
    class _StubWindow(EmulatorUIMixin):
        def __init__(self) -> None:
            pass

    def test_retroarch_core_list_path_respects_grid_launcher_share_dir(self) -> None:
        stub = self._StubWindow()
        with patch.dict(os.environ, {"GRID_LAUNCHER_SHARE_DIR": "/app/share/grid-launcher"}, clear=False):
            result = stub._retroarch_core_list_path()

        self.assertEqual(result, Path("/app/share/grid-launcher/retroarch-core-list.json"))

    def test_emulator_autoprofiles_path_respects_grid_launcher_share_dir(self) -> None:
        stub = self._StubWindow()
        with patch.dict(os.environ, {"GRID_LAUNCHER_SHARE_DIR": "/app/share/grid-launcher"}, clear=False):
            result = stub._emulator_autoprofiles_path()

        self.assertEqual(result, Path("/app/share/grid-launcher/emulator-autoprofiles.json"))


class RPCS3GamesYmlTests(unittest.TestCase):
    def test_prefers_games_dir_for_disc_layout_when_provided(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            games_dir = tmp_path / "games"
            (games_dir / "BLUS30336").mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0, games_dir)

            games_yml = data_root / "config" / "games.yml"
            self.assertTrue(result)
            content = games_yml.read_text(encoding="utf-8")
            self.assertIn((games_dir / "BLUS30336").as_posix(), content)

    def test_writes_new_games_yml_disc_layout(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            (dev_hdd0 / "game" / "BLUS30336").mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0)

            games_yml = data_root / "config" / "games.yml"
            self.assertTrue(result)
            self.assertTrue(games_yml.exists())
            content = games_yml.read_text(encoding="utf-8")
            self.assertIn("BLUS30336:", content)
            self.assertIn((dev_hdd0 / "game" / "BLUS30336").resolve().as_posix(), content)

    def test_writes_new_games_yml_hdd_layout(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            (dev_hdd0 / "game" / "BCUS12345").mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BCUS12345", dev_hdd0)

            self.assertTrue(result)
            content = (data_root / "config" / "games.yml").read_text(encoding="utf-8")
            self.assertIn("BCUS12345:", content)
            self.assertIn((dev_hdd0 / "game" / "BCUS12345").resolve().as_posix(), content)

    def test_writes_game_dir_when_game_dir_exists_without_eboot(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            game_dir = dev_hdd0 / "game" / "BLES01234"
            game_dir.mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BLES01234", dev_hdd0)

            self.assertTrue(result)
            content = (data_root / "config" / "games.yml").read_text(encoding="utf-8")
            self.assertIn("BLES01234:", content)
            self.assertIn(game_dir.resolve().as_posix(), content)

    def test_updates_existing_entry_same_game_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            config_dir = data_root / "config"
            config_dir.mkdir(parents=True)
            games_yml = config_dir / "games.yml"
            games_yml.write_text('BLUS30336: "/old/path/EBOOT.BIN"\n', encoding="utf-8")

            (dev_hdd0 / "game" / "BLUS30336").mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0)

            self.assertTrue(result)
            content = games_yml.read_text(encoding="utf-8")
            self.assertIn("BLUS30336:", content)
            self.assertNotIn('/old/path', content)
            self.assertEqual(content.count("BLUS30336:"), 1)
    def test_preserves_other_games_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            config_dir = data_root / "config"
            config_dir.mkdir(parents=True)
            games_yml = config_dir / "games.yml"
            games_yml.write_text('BCUS12345: "/some/path/EBOOT.BIN"\n', encoding="utf-8")

            (dev_hdd0 / "game" / "BLUS30336").mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0)

            self.assertTrue(result)
            content = games_yml.read_text(encoding="utf-8")
            self.assertIn('BCUS12345: "/some/path/EBOOT.BIN"', content)
            self.assertIn("BLUS30336:", content)

    def test_writes_game_dir_when_no_eboot_found(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            game_dir = dev_hdd0 / "game" / "BLUS30336"
            game_dir.mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0)

            self.assertTrue(result)
            games_yml = data_root / "config" / "games.yml"
            self.assertTrue(games_yml.exists())
            content = games_yml.read_text(encoding="utf-8")
            self.assertIn("BLUS30336:", content)
            self.assertIn(game_dir.resolve().as_posix(), content)
            self.assertNotIn("EBOOT.BIN", content)

    def test_returns_false_for_empty_game_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"

            result = update_rpcs3_games_yml(data_root, "", dev_hdd0)

            self.assertFalse(result)

    def test_creates_config_dir_if_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            (dev_hdd0 / "game" / "BLUS30336").mkdir(parents=True)

            result = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0)

            self.assertTrue(result)
            self.assertTrue((data_root / "config").exists())
            self.assertTrue((data_root / "config" / "games.yml").exists())

    def test_idempotent_on_repeat_call(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_root = tmp_path / "portable"
            dev_hdd0 = tmp_path / "dev_hdd0"
            (dev_hdd0 / "game" / "BLUS30336").mkdir(parents=True)

            result_first = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0)
            result_second = update_rpcs3_games_yml(data_root, "BLUS30336", dev_hdd0)

            content = (data_root / "config" / "games.yml").read_text(encoding="utf-8")
            self.assertTrue(result_first)
            self.assertTrue(result_second)
            self.assertEqual(content.count("BLUS30336:"), 1)


class RPCS3CustomConfigCopyTests(unittest.TestCase):
    def test_copies_files_when_source_exists(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            vfs_config = tmp_path / "vfs" / "config"
            (vfs_config / "custom_configs").mkdir(parents=True)
            (vfs_config / "custom_configs" / "config_BLUS30443.yml").write_bytes(b"config data")
            rpcs3_root = tmp_path / "rpcs3"

            copy_ps3_custom_config_to_emulator(vfs_config, rpcs3_root)

            dest = rpcs3_root / "config" / "custom_configs" / "config_BLUS30443.yml"
            self.assertTrue(dest.exists())
            self.assertEqual(dest.read_bytes(), b"config data")

    def test_skips_silently_when_source_dir_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            vfs_config = tmp_path / "vfs" / "config"  # custom_configs subdir doesn't exist
            rpcs3_root = tmp_path / "rpcs3"

            # Should not raise
            copy_ps3_custom_config_to_emulator(vfs_config, rpcs3_root)

            self.assertFalse((rpcs3_root / "config" / "custom_configs").exists())

    def test_creates_dest_dir_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            vfs_config = tmp_path / "vfs" / "config"
            (vfs_config / "custom_configs").mkdir(parents=True)
            (vfs_config / "custom_configs" / "config_BLUS30443.yml").write_bytes(b"x")
            rpcs3_root = tmp_path / "rpcs3"
            # rpcs3_root/config/custom_configs does not pre-exist

            copy_ps3_custom_config_to_emulator(vfs_config, rpcs3_root)

            self.assertTrue((rpcs3_root / "config" / "custom_configs").is_dir())

    def test_copies_multiple_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            vfs_config = tmp_path / "vfs" / "config"
            (vfs_config / "custom_configs").mkdir(parents=True)
            (vfs_config / "custom_configs" / "config_BLUS30443.yml").write_bytes(b"a")
            (vfs_config / "custom_configs" / "config_BCUS98174.yml").write_bytes(b"b")
            rpcs3_root = tmp_path / "rpcs3"

            copy_ps3_custom_config_to_emulator(vfs_config, rpcs3_root)

            dest_dir = rpcs3_root / "config" / "custom_configs"
            self.assertTrue((dest_dir / "config_BLUS30443.yml").exists())
            self.assertTrue((dest_dir / "config_BCUS98174.yml").exists())


class FlatpakEmulatorDetectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_flatpak_detection_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load grid-launcher.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    def test_ensure_emulator_sync_settings_uses_flatpak_config_root_for_ppsspp(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {
                    "retroachievements_username": "psp_user",
                    "retroachievements_token": "psp_token",
                }

            def _is_retroarch_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_duckstation_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_xemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_pcsx2_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[tuple[str, str]]:
                return []

            def _is_dolphin_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_azahar_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_eden_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_rpcs3_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_ppsspp_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return True

            def _is_cemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_redream_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

        window = _WindowStub()

        with patch("grid_launcher.ui.mixins.emulator_ui_mixin.ensure_ppsspp_settings") as ensure_ppsspp:
            module.MainWindow._ensure_emulator_sync_settings(
                window,
                "PPSSPP",
                "/home/user/.var/app/org.ppsspp.PPSSPP/exports/bin/flatpak",
                flatpak_config_root="/home/user/.var/app/org.ppsspp.PPSSPP/config/ppsspp",
            )

        ensure_ppsspp.assert_called_once_with(
            "/home/user/.var/app/org.ppsspp.PPSSPP/config/ppsspp",
            retroachievements_username="psp_user",
            retroachievements_token="psp_token",
        )

    def test_ensure_emulator_sync_settings_without_flatpak_config_root_uses_path_text(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {
                    "retroachievements_username": "psp_user",
                    "retroachievements_token": "psp_token",
                }

            def _is_retroarch_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_duckstation_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_xemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_pcsx2_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _resolved_firmware_directories(self, emulator_entry: dict[str, str]) -> list[tuple[str, str]]:
                return []

            def _is_dolphin_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_azahar_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_eden_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_rpcs3_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_ppsspp_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return True

            def _is_cemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_redream_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

        window = _WindowStub()

        with patch("grid_launcher.ui.mixins.emulator_ui_mixin.ensure_ppsspp_settings") as ensure_ppsspp:
            module.MainWindow._ensure_emulator_sync_settings(window, "PPSSPP", "C:/Emulators/PPSSPPWindows64.exe")

        ensure_ppsspp.assert_called_once_with(
            "C:/Emulators/PPSSPPWindows64.exe",
            retroachievements_username="psp_user",
            retroachievements_token="psp_token",
        )

    def test_trigger_flatpak_emulator_detection_background_noop_on_non_linux(self) -> None:
        module = type(self).module

        class _WindowStub:
            pass

        window = _WindowStub()

        with patch("sys.platform", "win32"), patch("threading.Thread") as thread_ctor:
            module.MainWindow._trigger_flatpak_emulator_detection_background(window)

        thread_ctor.assert_not_called()

    def test_trigger_flatpak_emulator_detection_background_filters_known_emulators(self) -> None:
        module = type(self).module
        emitted: list = []

        class _SignalStub:
            def emit(self, entries: list) -> None:
                emitted.append(entries)

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {
                    "emulators": [
                        {"name": "PPSSPP", "path": "/existing/ppsspp", "flatpak_app_id": "org.ppsspp.PPSSPP"},
                        {"name": "Dolphin", "path": "/existing/dolphin"},
                    ]
                }
                self._flatpak_detection_completed = _SignalStub()

            def _emulators(self) -> list[dict]:
                return self.config.get("emulators", [])

            def _emulator_autoprofiles(self) -> list[dict]:
                return []

        window = _WindowStub()

        detected = [
            {"name": "PPSSPP", "path": "/usr/bin/flatpak", "flatpak_app_id": "org.ppsspp.PPSSPP"},  # known by app id
            {"name": "Dolphin", "path": "/usr/bin/flatpak", "flatpak_app_id": "org.DolphinEmu.dolphin-emu"},  # known by name
            {"name": "PCSX2", "path": "/usr/bin/flatpak", "flatpak_app_id": "net.pcsx2.PCSX2"},  # new
        ]

        class _ThreadStub:
            def __init__(self, target=None, daemon=None) -> None:
                self._target = target

            def start(self) -> None:
                if self._target is not None:
                    self._target()

        with patch("sys.platform", "linux"), \
                patch("grid_launcher.ui.mixins.emulator_ui_mixin.detect_installed_flatpak_emulators", return_value=detected), \
                patch("threading.Thread", _ThreadStub):
            module.MainWindow._trigger_flatpak_emulator_detection_background(window)

        self.assertEqual(len(emitted), 1)
        new_entries = emitted[0]
        self.assertEqual(len(new_entries), 1)
        self.assertEqual(new_entries[0]["name"], "PCSX2")

    def test_on_flatpak_detection_completed_adds_entries(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {"emulators": []}
                self.sync_calls: list[tuple[str, str, str]] = []
                self.saved_config: dict | None = None
                self.refreshed = False
                self.toasts: list[dict] = []

            def _emulators(self) -> list[dict]:
                return self.config.get("emulators", [])

            def _emulator_autoprofiles(self) -> list[dict]:
                return []

            def _normalize_save_strategy_value(self, value: str) -> str:
                return "auto"

            def _normalize_emulators(self, value: list) -> list:
                return value

            def _ensure_emulator_sync_settings(self, name: str, path: str, *, flatpak_config_root: str = "") -> None:
                self.sync_calls.append((name, path, flatpak_config_root))

            def _refresh_emulator_views(self) -> None:
                self.refreshed = True

            def _save_config(self, config: dict) -> None:
                self.saved_config = config

            def _show_toast(self, payload: dict) -> None:
                self.toasts.append(payload)

            def _trigger_firmware_install_for_source_emulator(self, emulator_name: str) -> None:
                pass

            def _emulator_profile_for_entry(self, entry: dict) -> dict:
                return {}

            def _normalize_default_emulators(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _normalize_default_retroarch_cores(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _is_retroarch_emulator_name(self, name: str) -> bool:
                return False

            def _default_assignable_server_platforms(self) -> list:
                return []

            def _installed_retroarch_cores_for_platform(self, platform, name, emulator_entry=None) -> list:
                return []

            def _matching_platforms_for_emulator_keywords(self, keywords) -> list:
                return []

            def _dolphin_variant_label_for_game(self, game) -> str:
                return ""

            def _dolphin_target_platforms_for_variant(self, variant) -> list:
                return []

        window = _WindowStub()

        new_entries = [
            {
                "name": "PPSSPP",
                "path": "/usr/bin/flatpak",
                "args": "run org.ppsspp.PPSSPP",
                "flatpak_app_id": "org.ppsspp.PPSSPP",
                "_flatpak_config_root": "/home/user/.var/app/org.ppsspp.PPSSPP/config/ppsspp",
            },
            {
                "name": "PCSX2",
                "path": "/usr/bin/flatpak",
                "args": "run net.pcsx2.PCSX2",
                "flatpak_app_id": "net.pcsx2.PCSX2",
            },
        ]

        module.MainWindow._on_flatpak_detection_completed(window, new_entries)

        self.assertEqual(len(window.config["emulators"]), 2)
        self.assertIn(
            ("PPSSPP", "/usr/bin/flatpak", "/home/user/.var/app/org.ppsspp.PPSSPP/config/ppsspp"),
            window.sync_calls,
        )
        self.assertIn(("PCSX2", "/usr/bin/flatpak", ""), window.sync_calls)
        self.assertTrue(window.refreshed)
        self.assertIsNotNone(window.saved_config)
        self.assertEqual(len(window.toasts), 1)
        self.assertEqual(window.toasts[0]["level"], "success")
        for emulator in window.config["emulators"]:
            self.assertNotIn("_flatpak_config_root", emulator)

    def test_on_flatpak_detection_completed_assigns_default_emulator(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {"emulators": [], "default_emulators": {}}

            def _emulators(self) -> list[dict]:
                return self.config.get("emulators", [])

            def _emulator_autoprofiles(self) -> list[dict]:
                return []

            def _normalize_save_strategy_value(self, value: str) -> str:
                return "auto"

            def _normalize_emulators(self, value: list) -> list:
                return value

            def _ensure_emulator_sync_settings(self, name: str, path: str, *, flatpak_config_root: str = "") -> None:
                pass

            def _refresh_emulator_views(self) -> None:
                pass

            def _save_config(self, config: dict) -> None:
                pass

            def _show_toast(self, payload: dict) -> None:
                pass

            def _trigger_firmware_install_for_source_emulator(self, emulator_name: str) -> None:
                pass

            def _emulator_profile_for_entry(self, entry: dict) -> dict:
                return {"name": "PPSSPP", "platform_keywords": ["PlayStation Portable"]}

            def _normalize_default_emulators(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _normalize_default_retroarch_cores(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _is_retroarch_emulator_name(self, name: str) -> bool:
                return False

            def _default_assignable_server_platforms(self) -> list:
                return ["PlayStation Portable"]

            def _installed_retroarch_cores_for_platform(self, platform, name, emulator_entry=None) -> list:
                return []

            def _matching_platforms_for_emulator_keywords(self, keywords) -> list:
                return ["PlayStation Portable"]

            def _dolphin_variant_label_for_game(self, game) -> str:
                return ""

            def _dolphin_target_platforms_for_variant(self, variant) -> list:
                return []

        window = _WindowStub()

        new_entries = [
            {
                "name": "PPSSPP",
                "path": "/usr/bin/flatpak",
                "args": "run org.ppsspp.PPSSPP",
                "flatpak_app_id": "org.ppsspp.PPSSPP",
            },
        ]

        module.MainWindow._on_flatpak_detection_completed(window, new_entries)

        self.assertEqual(window.config["default_emulators"].get("PlayStation Portable"), "PPSSPP")

    def test_on_flatpak_detection_completed_does_not_overwrite_existing_default(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {"emulators": [], "default_emulators": {"PlayStation Portable": "MyPSP"}}

            def _emulators(self) -> list[dict]:
                return self.config.get("emulators", [])

            def _emulator_autoprofiles(self) -> list[dict]:
                return []

            def _normalize_save_strategy_value(self, value: str) -> str:
                return "auto"

            def _normalize_emulators(self, value: list) -> list:
                return value

            def _ensure_emulator_sync_settings(self, name: str, path: str, *, flatpak_config_root: str = "") -> None:
                pass

            def _refresh_emulator_views(self) -> None:
                pass

            def _save_config(self, config: dict) -> None:
                pass

            def _show_toast(self, payload: dict) -> None:
                pass

            def _trigger_firmware_install_for_source_emulator(self, emulator_name: str) -> None:
                pass

            def _emulator_profile_for_entry(self, entry: dict) -> dict:
                return {"name": "PPSSPP", "platform_keywords": ["PlayStation Portable"]}

            def _normalize_default_emulators(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _normalize_default_retroarch_cores(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _is_retroarch_emulator_name(self, name: str) -> bool:
                return False

            def _default_assignable_server_platforms(self) -> list:
                return ["PlayStation Portable"]

            def _installed_retroarch_cores_for_platform(self, platform, name, emulator_entry=None) -> list:
                return []

            def _matching_platforms_for_emulator_keywords(self, keywords) -> list:
                return ["PlayStation Portable"]

            def _dolphin_variant_label_for_game(self, game) -> str:
                return ""

            def _dolphin_target_platforms_for_variant(self, variant) -> list:
                return []

        window = _WindowStub()

        new_entries = [
            {
                "name": "PPSSPP",
                "path": "/usr/bin/flatpak",
                "args": "run org.ppsspp.PPSSPP",
                "flatpak_app_id": "org.ppsspp.PPSSPP",
            },
        ]

        module.MainWindow._on_flatpak_detection_completed(window, new_entries)

        self.assertEqual(window.config["default_emulators"]["PlayStation Portable"], "MyPSP")

    def test_on_flatpak_detection_completed_assigns_retroarch_core_default(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {"emulators": [], "default_emulators": {}, "default_retroarch_cores": {}}

            def _emulators(self) -> list[dict]:
                return self.config.get("emulators", [])

            def _emulator_autoprofiles(self) -> list[dict]:
                return []

            def _normalize_save_strategy_value(self, value: str) -> str:
                return "auto"

            def _normalize_emulators(self, value: list) -> list:
                return value

            def _ensure_emulator_sync_settings(self, name: str, path: str, *, flatpak_config_root: str = "") -> None:
                pass

            def _refresh_emulator_views(self) -> None:
                pass

            def _save_config(self, config: dict) -> None:
                pass

            def _show_toast(self, payload: dict) -> None:
                pass

            def _trigger_firmware_install_for_source_emulator(self, emulator_name: str) -> None:
                pass

            def _emulator_profile_for_entry(self, entry: dict) -> dict:
                return {
                    "name": "RetroArch (Multi-System)",
                    "platform_keywords": ["Game Boy Advance"],
                    "all_platforms": False,
                }

            def _normalize_default_emulators(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _normalize_default_retroarch_cores(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _is_retroarch_emulator_name(self, name: str) -> bool:
                return True

            def _default_assignable_server_platforms(self) -> list:
                return ["Game Boy Advance"]

            def _installed_retroarch_cores_for_platform(self, platform, name, emulator_entry=None) -> list:
                return ["mgba"]

            def _matching_platforms_for_emulator_keywords(self, keywords) -> list:
                return ["Game Boy Advance"]

            def _dolphin_variant_label_for_game(self, game) -> str:
                return ""

            def _dolphin_target_platforms_for_variant(self, variant) -> list:
                return []

        window = _WindowStub()

        new_entries = [
            {
                "name": "RetroArch (Multi-System)",
                "path": "/usr/bin/flatpak",
                "args": "run org.libretro.RetroArch",
                "flatpak_app_id": "org.libretro.RetroArch",
            },
        ]

        module.MainWindow._on_flatpak_detection_completed(window, new_entries)

        self.assertEqual(window.config["default_retroarch_cores"].get("Game Boy Advance"), "mgba")

    def test_on_flatpak_detection_completed_empty_list_is_noop(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.save_called = False
                self.refresh_called = False

            def _save_config(self, config: dict) -> None:
                self.save_called = True

            def _refresh_emulator_views(self) -> None:
                self.refresh_called = True

        window = _WindowStub()
        module.MainWindow._on_flatpak_detection_completed(window, [])

        self.assertFalse(window.save_called)
        self.assertFalse(window.refresh_called)


class BackfillMissingEmulatorDefaultsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_backfill_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load grid-launcher.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    @staticmethod
    def _make_window_stub(config: dict):
        class _WindowStub:
            def __init__(self) -> None:
                self.config = config
                self.save_called = False

            def _save_config(self, cfg: dict) -> None:
                self.save_called = True

            def _emulator_profile_for_entry(self, entry: dict) -> dict:
                return {"name": "PPSSPP", "platform_keywords": ["PlayStation Portable"]}

            def _normalize_default_emulators(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _normalize_default_retroarch_cores(self, value: dict) -> dict:
                return value if isinstance(value, dict) else {}

            def _is_retroarch_emulator_name(self, name: str) -> bool:
                return False

            def _default_assignable_server_platforms(self) -> list:
                return ["PlayStation Portable"]

            def _installed_retroarch_cores_for_platform(self, platform, name, emulator_entry=None) -> list:
                return []

            def _matching_platforms_for_emulator_keywords(self, keywords) -> list:
                return ["PlayStation Portable"]

            def _dolphin_variant_label_for_game(self, game) -> str:
                return ""

            def _dolphin_target_platforms_for_variant(self, variant) -> list:
                return []

        return _WindowStub()

    def test_backfill_assigns_default_for_emulator_with_no_default(self) -> None:
        module = type(self).module
        window = self._make_window_stub({
            "emulators": [{"name": "PPSSPP", "path": "/usr/bin/ppsspp"}],
            "default_emulators": {},
            "default_retroarch_cores": {},
        })

        module.MainWindow._backfill_missing_emulator_defaults(window)

        self.assertEqual(window.config["default_emulators"].get("PlayStation Portable"), "PPSSPP")
        self.assertTrue(window.save_called)

    def test_backfill_does_not_overwrite_existing_default(self) -> None:
        module = type(self).module
        window = self._make_window_stub({
            "emulators": [{"name": "PPSSPP", "path": "/usr/bin/ppsspp"}],
            "default_emulators": {"PlayStation Portable": "AnotherEmulator"},
            "default_retroarch_cores": {},
        })

        module.MainWindow._backfill_missing_emulator_defaults(window)

        self.assertEqual(window.config["default_emulators"]["PlayStation Portable"], "AnotherEmulator")
        self.assertFalse(window.save_called)


class RetroarchCoreArgumentPathTests(unittest.TestCase):
    def test_bare_name_on_windows(self) -> None:
        with patch("sys.platform", "win32"):
            self.assertEqual(retroarch_core_argument_path("snes9x"), "cores/snes9x_libretro.dll")

    def test_bare_name_on_linux(self) -> None:
        with patch("sys.platform", "linux"):
            self.assertEqual(retroarch_core_argument_path("snes9x"), "cores/snes9x_libretro.so")

    def test_bare_name_on_macos(self) -> None:
        with patch("sys.platform", "darwin"):
            self.assertEqual(retroarch_core_argument_path("snes9x"), "cores/snes9x_libretro.dylib")

    def test_libretro_suffix_on_linux(self) -> None:
        with patch("sys.platform", "linux"):
            self.assertEqual(retroarch_core_argument_path("snes9x_libretro"), "cores/snes9x_libretro.so")

    def test_dll_extension_reextensioned_on_linux(self) -> None:
        with patch("sys.platform", "linux"):
            self.assertEqual(retroarch_core_argument_path("snes9x_libretro.dll"), "cores/snes9x_libretro.so")

    def test_so_extension_reextensioned_on_windows(self) -> None:
        with patch("sys.platform", "win32"):
            self.assertEqual(retroarch_core_argument_path("snes9x_libretro.so"), "cores/snes9x_libretro.dll")

    def test_full_path_returned_as_is(self) -> None:
        with patch("sys.platform", "linux"):
            self.assertEqual(
                retroarch_core_argument_path("/opt/cores/snes9x_libretro.dll"),
                "/opt/cores/snes9x_libretro.dll",
            )

    def test_empty_string(self) -> None:
        with patch("sys.platform", "linux"):
            self.assertEqual(retroarch_core_argument_path(""), "")


if __name__ == "__main__":
    unittest.main()