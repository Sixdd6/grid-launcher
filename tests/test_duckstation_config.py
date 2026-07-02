from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from grid_launcher.emulator.duckstation import (
    duckstation_config_path_candidates,
    duckstation_memory_card_settings,
    ensure_duckstation_memory_card_settings,
)


class DuckStationConfigTests(unittest.TestCase):
    def test_ensure_duckstation_memory_card_settings_forces_per_game_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "\n".join(
                    [
                        "[MemoryCards]",
                        "Card1Type = Shared",
                        "Card2Type = Shared",
                        "UsePlaylistTitle = false",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            settings = duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertEqual(settings["card1_type"], "PerGameTitle")
        self.assertEqual(settings["card2_type"], "None")
        self.assertTrue(settings["use_playlist_title"])
        self.assertEqual(settings["directory"], "memcards")
        self.assertIn("[MemoryCards]", text)
        self.assertIn("Card1Type = PerGameTitle", text)
        self.assertIn("Card2Type = None", text)
        self.assertIn("UsePlaylistTitle = true", text)
        self.assertIn("Directory = memcards", text)

    def test_ensure_duckstation_memory_card_settings_preserves_explicit_memcard_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "\n".join(
                    [
                        "[MemoryCards]",
                        "Directory = D:/CustomMemcards",
                        "Card1Type = Shared",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            settings = duckstation_memory_card_settings(str(emulator_path))

        self.assertTrue(result["changed"])
        self.assertEqual(settings["directory"], "D:/CustomMemcards")
        self.assertEqual(settings["card1_type"], "PerGameTitle")
        self.assertEqual(settings["card2_type"], "None")

    def test_ensure_duckstation_memory_card_settings_enables_fullscreen_and_cheevos_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "\n".join(
                    [
                        "[Main]",
                        "StartFullscreen = false",
                        "[Achievements]",
                        "Enabled = false",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(
                str(emulator_path),
                enable_fullscreen=True,
            )
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("StartFullscreen = true", text)
        self.assertIn("Enabled = true", text)
        self.assertIn("ChallengeMode = false", text)
        self.assertIn("LeaderboardNotifications = false", text)
        self.assertIn("LeaderboardTrackers = false", text)

    def test_ensure_duckstation_writes_autoupdater_disabled(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[AutoUpdater]", text)
        self.assertIn("CheckAtStartup = false", text)

    def test_ensure_duckstation_writes_default_resolution_scale(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[GPU]", text)
        self.assertIn("ResolutionScale = 4", text)

    def test_ensure_duckstation_preserves_user_resolution_scale(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[GPU]\nResolutionScale = 2\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("ResolutionScale = 2", text)
        self.assertNotIn("ResolutionScale = 4", text)

    def test_gpu_video_defaults_set_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[Main]\n",
                encoding="utf-8",
            )

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("PGXPEnable = true", text)
        self.assertIn("PGXPColorCorrection = true", text)
        self.assertIn("TextureFilter = Scale2x", text)
        self.assertIn("SpriteTextureFilter = Scale2x", text)
        self.assertIn("DitheringMode = TrueColorFull", text)
        self.assertIn("LineDetectMode = BasicTriangles", text)
        self.assertIn("DownsampleMode = Box", text)
        self.assertIn("DownsampleScale = 2", text)
        self.assertIn("ResolutionScale = 4", text)

    def test_gpu_video_defaults_not_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[GPU]\nTextureFilter = Nearest\nPGXPEnable = false\n",
                encoding="utf-8",
            )

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        lines = text.splitlines()

        self.assertIn("TextureFilter = Nearest", lines)
        self.assertIn("PGXPEnable = false", lines)
        self.assertNotIn("TextureFilter = Scale2x", lines)
        self.assertNotIn("PGXPEnable = true", lines)
        self.assertIn("DitheringMode = TrueColorFull", text)
        self.assertIn("DownsampleMode = Box", text)

    def test_display_scaling_defaults_set_when_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[Main]\n",
                encoding="utf-8",
            )

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("Scaling = Lanczos", text)
        self.assertIn("Scaling24Bit = Lanczos", text)

    def test_display_scaling_not_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[Display]\nScaling = Bilinear\nScaling24Bit = Bilinear\n",
                encoding="utf-8",
            )

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("Scaling = Bilinear", text)
        self.assertIn("Scaling24Bit = Bilinear", text)
        self.assertNotIn("Scaling = Lanczos", text)
        self.assertNotIn("Scaling24Bit = Lanczos", text)

    def test_ensure_duckstation_writes_default_output_volume(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[Audio]", text)
        self.assertIn("OutputVolume = 60", text)

    def test_ensure_duckstation_preserves_user_output_volume(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[Audio]\nOutputVolume = 80\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("OutputVolume = 80", text)
        self.assertNotIn("OutputVolume = 60", text)

    def test_ensure_duckstation_writes_pause_menu_hotkey(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[Hotkeys]", text)
        self.assertIn("OpenPauseMenu = SDL-0/Guide", text)

    def test_ensure_duckstation_preserves_user_pause_menu_hotkey(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[Hotkeys]\nOpenPauseMenu = Keyboard/Escape\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("OpenPauseMenu = Keyboard/Escape", text)
        self.assertNotIn("OpenPauseMenu = SDL-0/Guide", text)

    def test_ensure_duckstation_writes_default_pad1_controller(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[Pad1]", text)
        self.assertIn("Type = AnalogController", text)
        self.assertIn("Cross = SDL-0/A", text)
        self.assertIn("Circle = SDL-0/B", text)
        self.assertIn("Square = SDL-0/X", text)
        self.assertIn("Triangle = SDL-0/Y", text)
        self.assertIn("LUp = SDL-0/-LeftY", text)

    def test_ensure_duckstation_preserves_user_pad1_controller(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[Pad1]\nType = DigitalController\nCross = Keyboard/Z\n",
                encoding="utf-8",
            )

            ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertIn("Type = DigitalController", text)
        self.assertNotIn("Type = AnalogController", text)
        self.assertIn("Cross = Keyboard/Z", text)
        self.assertNotIn("Cross = SDL-0/A", text)

    def test_ensure_duckstation_writes_setup_wizard_incomplete_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("SetupWizardIncomplete = false", text)

    def test_ensure_duckstation_overrides_setup_wizard_incomplete_true(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            duckstation_dir = Path(temp_dir) / "DuckStation"
            duckstation_dir.mkdir()
            emulator_path = duckstation_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            config_path = duckstation_dir / "settings.ini"
            config_path.write_text(
                "[Main]\nSetupWizardIncomplete = true\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            text = config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("SetupWizardIncomplete = false", text)
        self.assertNotIn("SetupWizardIncomplete = true", text)

    def test_ensure_duckstation_preserves_portable_cheevos_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "DuckStationPortable"
            emulator_dir.mkdir()
            emulator_path = emulator_dir / "duckstation-qt-x64-ReleaseLTCG.exe"
            emulator_path.write_bytes(b"")
            portable_config_path = emulator_dir / "settings.ini"
            portable_config_path.write_text(
                "\n".join(
                    [
                        "[Cheevos]",
                        "Enabled = true",
                        "Username = portable_user",
                        "Token = portable_token",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            result = ensure_duckstation_memory_card_settings(str(emulator_path))
            portable_text = portable_config_path.read_text(encoding="utf-8")

        self.assertTrue(result["changed"])
        self.assertIn("[Cheevos]", portable_text)
        self.assertIn("Enabled = true", portable_text)
        self.assertIn("ChallengeMode = false", portable_text)
        self.assertIn("LeaderboardNotifications = false", portable_text)
        self.assertIn("LeaderboardTrackers = false", portable_text)
        self.assertIn("Username = portable_user", portable_text)
        self.assertIn("Token = portable_token", portable_text)

    def test_config_path_candidates_falls_back_to_dotfile_dirs_when_xdg_unset(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("XDG_DATA_HOME", None)
            os.environ.pop("XDG_CONFIG_HOME", None)
            candidates = duckstation_config_path_candidates("/nonexistent/duckstation.exe")

        home = Path.home()
        self.assertIn(home / ".local" / "share" / "duckstation" / "settings.ini", candidates)
        self.assertIn(home / ".config" / "duckstation" / "settings.ini", candidates)

    def test_config_path_candidates_honors_xdg_data_home_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xdg_data_home = Path(temp_dir) / "xdg-data"
            with patch.dict(os.environ, {"XDG_DATA_HOME": str(xdg_data_home)}, clear=False):
                candidates = duckstation_config_path_candidates("/nonexistent/duckstation.exe")

        self.assertIn(xdg_data_home / "duckstation" / "settings.ini", candidates)

    def test_config_path_candidates_honors_xdg_config_home_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            xdg_config_home = Path(temp_dir) / "xdg-config"
            with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(xdg_config_home)}, clear=False):
                candidates = duckstation_config_path_candidates("/nonexistent/duckstation.exe")

        self.assertIn(xdg_config_home / "duckstation" / "settings.ini", candidates)

if __name__ == "__main__":
    unittest.main()
