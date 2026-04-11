from __future__ import annotations

import importlib.util
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import json

from rom_mate.emulator.azahar import ensure_azahar_settings
from rom_mate.emulator.cemu import ensure_cemu_settings
from rom_mate.emulator import ensure_dolphin_settings, ensure_dolphin_skip_ipl, ensure_dolphin_gcpad_config
from rom_mate.emulator.duckstation import ensure_duckstation_memory_card_settings
from rom_mate.emulator.eden import ensure_eden_settings
from rom_mate.emulator.pcsx2 import ensure_pcsx2_settings, pcsx2_data_root_candidates
from rom_mate.emulator.ppsspp import ensure_ppsspp_settings
from rom_mate.emulator.redream import ensure_redream_settings
from rom_mate.emulator.retroarch import ensure_retroarch_save_location_settings
from rom_mate.emulator.xemu import ensure_xemu_settings


class EmulatorAutoConfigSettingsTests(unittest.TestCase):
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
            with patch("rom_mate.emulator.pcsx2._windows_documents_folder", return_value=fake_docs):
                candidates = pcsx2_data_root_candidates("", "", lambda s: [])
        self.assertIn(fake_docs / "PCSX2", candidates)

    def test_pcsx2_windows_documents_folder_exported_from_module(self) -> None:
        from rom_mate.emulator import pcsx2_windows_documents_folder

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
                result = ensure_dolphin_skip_ipl("")
        self.assertIsNotNone(result["dolphin_ini_path"])

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
                result = ensure_dolphin_gcpad_config("")

        self.assertIsNotNone(result["gcpad_ini_path"])

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

    def test_eden_writes_telemetry_and_discord(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            eden_dir = Path(temp_dir) / "Eden"
            eden_dir.mkdir()
            emulator_path = eden_dir / "eden.exe"
            emulator_path.write_bytes(b"")
            config_path = eden_dir / "qt-config.ini"
            config_path.write_text("", encoding="utf-8")

            ensure_eden_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("[UI]", text)
        self.assertIn("enable_discord_presence = false", text)
        self.assertIn("confirm_before_closing = false", text)
        self.assertIn("[WebService]", text)
        self.assertIn("enable_telemetry = false", text)

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

    def test_ppsspp_no_change_when_installed_txt_absent(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "PPSSPP"
            emulator_dir.mkdir()
            exe_path = emulator_dir / "PPSSPPWindows64.exe"
            exe_path.write_bytes(b"")

            result = ensure_ppsspp_settings(str(exe_path))

        self.assertFalse(result["changed"])

    def test_ppsspp_no_change_for_empty_path(self) -> None:
        result = ensure_ppsspp_settings("")

        self.assertFalse(result["changed"])

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
        self.assertIn("<check_update>false</check_update>", text)
        self.assertIn("<discord_presence>false</discord_presence>", text)

    def test_cemu_overwrites_check_update_true(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "settings.xml"
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<check_update>true</check_update>",
                        "<discord_presence>false</discord_presence>",
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

    def test_cemu_no_change_when_already_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            cemu_dir = Path(temp_dir) / "Cemu"
            cemu_dir.mkdir()
            emulator_path = cemu_dir / "cemu.exe"
            emulator_path.write_bytes(b"")
            config_path = cemu_dir / "settings.xml"
            config_path.write_text(
                "\n".join(
                    [
                        '<?xml version="1.0" encoding="utf-8"?>',
                        "<content>",
                        "<check_update>false</check_update>",
                        "<discord_presence>false</discord_presence>",
                        "</content>",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_cemu_settings(str(emulator_path))

        self.assertFalse(result["changed"])


class EmulatorEnsureDispatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        module_path = Path(__file__).resolve().parents[1] / "rom-mate.py"
        spec = importlib.util.spec_from_file_location("rom_mate_main_for_ensure_dispatch_tests", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load rom-mate.py for tests.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        cls.module = module

    def test_ensure_emulator_sync_settings_calls_ppsspp_settings(self) -> None:
        module = type(self).module

        class _WindowStub:
            def __init__(self) -> None:
                self.config = {}

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

            def _is_ppsspp_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return True

            def _is_cemu_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

            def _is_redream_emulator_name(self, emulator_name: str, emulator: dict[str, str] | None = None) -> bool:
                return False

        window = _WindowStub()

        with patch.object(module, "resolve_ensure_ppsspp_settings") as ensure_ppsspp:
            module.MainWindow._ensure_emulator_sync_settings(window, "PPSSPP", "C:/Emulators/PPSSPPWindows64.exe")

        ensure_ppsspp.assert_called_once_with("C:/Emulators/PPSSPPWindows64.exe")


if __name__ == "__main__":
    unittest.main()