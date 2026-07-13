from __future__ import annotations

import json
import os
import tempfile
import unittest
from inspect import signature
from pathlib import Path
from unittest.mock import patch

from grid_launcher.emulator import autoconfig as emulator_autoconfig
from grid_launcher.emulator.azahar import (
    azahar_directory_settings,
    azahar_save_path_overrides,
    azahar_state_path_overrides,
)
from grid_launcher.emulator.cemu import cemu_save_path_overrides
from grid_launcher.emulator.eden import (
    eden_directory_settings,
    eden_save_path_overrides,
)
from grid_launcher.emulator.fbneo import (
    fbneo_directory_settings,
    fbneo_save_path_overrides,
    fbneo_state_path_overrides,
)
from grid_launcher.emulator.dolphin import (
    dolphin_directory_settings,
    dolphin_save_path_overrides,
    dolphin_state_path_overrides,
)
from grid_launcher.emulator.mame import (
    mame_directory_settings,
    mame_save_path_overrides,
    mame_state_path_overrides,
)
from grid_launcher.emulator.pcsx2 import (
    pcsx2_directory_settings,
    pcsx2_save_path_overrides,
    pcsx2_state_path_overrides,
)
from grid_launcher.emulator.pico8 import (
    pico8_directory_settings,
    pico8_save_path_overrides,
)
from grid_launcher.emulator.redream import (
    redream_directory_settings,
    redream_save_path_overrides,
    redream_state_path_overrides,
)
from grid_launcher.emulator.rpcs3 import (
    rpcs3_directory_settings,
    rpcs3_save_path_overrides,
)
from grid_launcher.emulator.xemu import (
    xemu_directory_settings,
    xemu_save_path_overrides,
)
from grid_launcher.emulator.xenia import (
    _is_edge_variant,
    xenia_directory_settings,
    xenia_save_path_overrides,
    xenia_state_path_overrides,
)
from grid_launcher.emulator.profiles import (
    emulator_entry_matches_tokens,
    emulator_profile_for_game,
    load_emulator_autoprofiles,
    matching_platforms_for_emulator_keywords,
    normalize_ignore_extension_value,
    normalize_save_strategy_value,
)
from grid_launcher.emulator.selection import (
    cloud_save_block_reason_for_game,
    install_block_reason_for_game,
    is_xbox360_platform,
    is_native_executable_platform,
)


def _resolve_autoconfig_helper(*candidate_names: str):
    for name in candidate_names:
        helper = getattr(emulator_autoconfig, name, None)
        if callable(helper):
            return helper
    raise AssertionError(
        "Expected one of the manual auto-populate helpers to exist in grid_launcher.emulator.autoconfig: "
        + ", ".join(candidate_names)
    )


def _call_with_supported_kwargs(helper, **kwargs):
    helper_params = signature(helper).parameters
    supported_kwargs = {name: value for name, value in kwargs.items() if name in helper_params}
    return helper(**supported_kwargs)


def _coerce_manual_entry_result(result):
    if isinstance(result, dict):
        return result
    if isinstance(result, tuple):
        for item in result:
            if isinstance(item, dict) and "path" in item:
                return item
            if isinstance(item, list) and item and isinstance(item[0], dict) and "path" in item[0]:
                return item[0]
    raise AssertionError(f"Unable to extract manual emulator entry from helper result: {result!r}")


def _coerce_suggestion_names(result):
    if isinstance(result, tuple):
        for item in result:
            if isinstance(item, list):
                result = item
                break

    if not isinstance(result, list):
        raise AssertionError(f"Unable to extract suggestion list from helper result: {result!r}")

    names = []
    for item in result:
        if isinstance(item, str) and item.strip():
            names.append(item.strip())
            continue
        if isinstance(item, dict):
            name_value = item.get("name", "")
            if isinstance(name_value, str) and name_value.strip():
                names.append(name_value.strip())
    return names


class EmulatorAutoprofilesLoadingTests(unittest.TestCase):
    def test_is_xbox360_platform_detects_standard_label(self) -> None:
        self.assertTrue(is_xbox360_platform({"platform": "Xbox 360"}))

    def test_is_xbox360_platform_detects_compact_label(self) -> None:
        self.assertTrue(is_xbox360_platform({"platform": "Xbox360"}))

    def test_is_xbox360_platform_rejects_original_xbox(self) -> None:
        self.assertFalse(is_xbox360_platform({"platform": "Xbox"}))

    def test_is_xbox360_platform_rejects_xbox_one(self) -> None:
        self.assertFalse(is_xbox360_platform({"platform": "Xbox One"}))

    def test_select_emulator_executable_path_prefers_azahar_over_azahar_room(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            extracted_dir = Path(temp_dir) / "azahar"
            extracted_dir.mkdir()
            azahar_room_path = extracted_dir / "azahar-room.exe"
            azahar_path = extracted_dir / "azahar.exe"
            azahar_room_path.write_text("", encoding="utf-8")
            azahar_path.write_text("", encoding="utf-8")

            selected_path = emulator_autoconfig.select_emulator_executable_path(
                {
                    "title": "Nintendo 3DS",
                    "extracted_path": str(azahar_room_path),
                    "extracted_dir": str(extracted_dir),
                },
                Path(temp_dir) / "archive.zip",
                launchable_emulator_file=lambda path: path.suffix.casefold() == ".exe",
            )

        self.assertEqual(selected_path, str(azahar_path))

    def test_cemu_save_path_overrides_reads_custom_mlc_from_settings_xml(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "cemu"
            emulator_dir.mkdir()
            mlc_dir = Path(temp_dir) / "custom-mlc"
            (emulator_dir / "settings.xml").write_text(
                f"<content><mlc_path>{mlc_dir.as_posix()}</mlc_path></content>",
                encoding="utf-8",
            )

            overrides = cemu_save_path_overrides(str(emulator_dir / "cemu.exe"), "", lambda value: [value])

        self.assertEqual(overrides, [str(mlc_dir / "usr" / "save")])

    def test_pcsx2_directory_settings_reads_user_documents_config(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            fake_home = Path(temp_dir)
            config_dir = fake_home / "Documents" / "PCSX2" / "inis"
            config_dir.mkdir(parents=True)
            (config_dir / "PCSX2.ini").write_text(
                "[Folders]\nMemoryCards = custom-memcards\nSavestates = custom-sstates\n",
                encoding="utf-8",
            )

            with patch.dict(
                os.environ,
                {"USERPROFILE": str(fake_home), "HOME": str(fake_home), "OneDrive": ""},
                clear=False,
            ):
                with patch("grid_launcher.emulator.pcsx2.Path.home", return_value=fake_home):
                    with patch("grid_launcher.emulator.pcsx2._windows_documents_folder", return_value=None):
                        settings = pcsx2_directory_settings(
                            r"C:\Emulators\PCSX2\pcsx2-qt.exe",
                            "",
                            lambda value: [value],
                        )

        self.assertEqual(settings["memory_cards"], str(fake_home.resolve() / "Documents" / "PCSX2" / "custom-memcards"))
        self.assertEqual(settings["savestates"], str(fake_home / "Documents" / "PCSX2" / "custom-sstates"))

    def test_pcsx2_save_path_overrides_use_portable_config_values(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "pcsx2"
            emulator_dir.mkdir()
            (emulator_dir / "portable.ini").write_text("", encoding="utf-8")
            config_dir = emulator_dir / "inis"
            config_dir.mkdir()
            (config_dir / "PCSX2.ini").write_text(
                "[Folders]\nMemoryCards = memcards-custom\nSavestates = states-custom\n"
                "[MemoryCards]\nSlot1_Filename = custom-slot-1.ps2\nSlot2_Filename = custom-slot-2.ps2\n",
                encoding="utf-8",
            )

            save_overrides = pcsx2_save_path_overrides(
                str(emulator_dir / "pcsx2-qt.exe"),
                "-portable -fullscreen \"%rom%\"",
                str.split,
            )
            state_overrides = pcsx2_state_path_overrides(
                str(emulator_dir / "pcsx2-qt.exe"),
                "-portable -fullscreen \"%rom%\"",
                str.split,
            )

        self.assertEqual(
            save_overrides,
            [
                str(emulator_dir.resolve() / "memcards-custom" / "custom-slot-1.ps2"),
                str(emulator_dir.resolve() / "memcards-custom" / "custom-slot-2.ps2"),
                str(emulator_dir.resolve() / "memcards-custom"),
            ],
        )
        self.assertEqual(state_overrides, [str(emulator_dir.resolve() / "states-custom")])

    def test_pcsx2_windows_documents_folder_is_public_wrapper(self) -> None:
        sentinel = Path("/fake/documents")
        with patch("grid_launcher.emulator.pcsx2._windows_documents_folder", return_value=sentinel):
            from grid_launcher.emulator.pcsx2 import pcsx2_windows_documents_folder

            result = pcsx2_windows_documents_folder()
        self.assertEqual(result, sentinel)

    def test_rpcs3_directory_settings_reads_vfs_and_persistent_active_user(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "rpcs3"
            (emulator_dir / "config").mkdir(parents=True)
            (emulator_dir / "GuiConfigs").mkdir()
            custom_hdd0 = Path(temp_dir) / "rpcs3-data" / "dev_hdd0"
            (custom_hdd0 / "home" / "00000002" / "savedata").mkdir(parents=True)

            (emulator_dir / "config" / "vfs.yml").write_text(
                '$(EmulatorDir): ""\n"/dev_hdd0/": "../rpcs3-data/dev_hdd0/"\n',
                encoding="utf-8",
            )
            (emulator_dir / "GuiConfigs" / "persistent_settings.dat").write_text(
                "[Users]\nactive_user=00000002\n",
                encoding="utf-8",
            )

            settings = rpcs3_directory_settings(
                str(emulator_dir / "rpcs3.exe"),
                '--no-gui "%RPCS3_GAMEID%:%ps3_gameid%"',
                str.split,
            )

        self.assertEqual(settings["current_user"], "00000002")
        self.assertEqual(settings["dev_hdd0"], str(custom_hdd0.resolve()))

    def test_rpcs3_save_path_overrides_prioritize_cli_user_and_existing_users(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "rpcs3"
            (emulator_dir / "config").mkdir(parents=True)
            (emulator_dir / "GuiConfigs").mkdir()
            custom_hdd0 = Path(temp_dir) / "portable-data" / "dev_hdd0"
            cli_user_path = custom_hdd0 / "home" / "00000003" / "savedata"
            persistent_user_path = custom_hdd0 / "home" / "00000002" / "savedata"
            cli_user_path.mkdir(parents=True)
            persistent_user_path.mkdir(parents=True)

            (emulator_dir / "config" / "vfs.yml").write_text(
                '$(EmulatorDir): ""\n"/dev_hdd0/": "../portable-data/dev_hdd0/"\n',
                encoding="utf-8",
            )
            (emulator_dir / "GuiConfigs" / "persistent_settings.dat").write_text(
                "[Users]\nactive_user=00000002\n",
                encoding="utf-8",
            )

            overrides = rpcs3_save_path_overrides(
                str(emulator_dir / "rpcs3.exe"),
                '--no-gui --user-id 00000003 "%RPCS3_GAMEID%:%ps3_gameid%"',
                str.split,
            )

        self.assertEqual(overrides[0], str(cli_user_path.resolve()))
        self.assertIn(str(persistent_user_path.resolve()), overrides)

    def test_dolphin_directory_settings_use_cli_user_and_configured_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "dolphin"
            emulator_dir.mkdir()
            user_dir = Path(temp_dir) / "custom-user"
            config_dir = user_dir / "Config"
            config_dir.mkdir(parents=True)
            (config_dir / "Dolphin.ini").write_text(
                "[Core]\n"
                "MemcardAPath = GC/MemoryCardA.custom.raw\n"
                "GCIFolderAPath = GCIBase\n"
                "[General]\n"
                "NANDRootPath = AltWii\n",
                encoding="utf-8",
            )

            settings = dolphin_directory_settings(
                str(emulator_dir / "Dolphin.exe"),
                f'-u "{user_dir}" -b -e "%rom%"',
                str.split,
            )

        self.assertEqual(settings["user_root"], str(user_dir.resolve()))
        self.assertEqual(settings["wii_root"], str((user_dir / "AltWii").resolve()))
        self.assertEqual(settings["memcard_a_path"], str((user_dir / "GC" / "MemoryCardA.custom.raw").resolve()))
        self.assertEqual(settings["gci_folder_a_path"], str((user_dir / "GCIBase").resolve()))

    def test_dolphin_save_path_overrides_include_gc_wii_and_states(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "dolphin"
            emulator_dir.mkdir()
            (emulator_dir / "portable.txt").write_text("", encoding="utf-8")
            config_dir = emulator_dir / "User" / "Config"
            config_dir.mkdir(parents=True)
            (config_dir / "Dolphin.ini").write_text(
                "[Core]\n"
                "MemcardAPath = GC/MemoryCardA.custom.raw\n"
                "GCIFolderAPathOverride = GC/OverrideA\n"
                "[General]\n"
                "NANDRootPath = AltWii\n",
                encoding="utf-8",
            )

            save_overrides = dolphin_save_path_overrides(
                str(emulator_dir / "Dolphin.exe"),
                '-b -e "%rom%"',
                str.split,
            )
            state_overrides = dolphin_state_path_overrides(
                str(emulator_dir / "Dolphin.exe"),
                '-b -e "%rom%"',
                str.split,
            )

        self.assertIn(str((emulator_dir / "User" / "GC" / "MemoryCardA.custom.raw").resolve()), save_overrides)
        self.assertIn(str((emulator_dir / "User" / "GC" / "OverrideA").resolve()), save_overrides)
        self.assertIn(str((emulator_dir / "User" / "AltWii" / "title").resolve()), save_overrides)
        self.assertEqual(state_overrides, [str((emulator_dir / "User" / "StateSaves").resolve())])

    def test_azahar_directory_settings_read_custom_storage_from_qt_config(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "azahar"
            config_dir = emulator_dir / "user" / "config"
            config_dir.mkdir(parents=True)
            (config_dir / "qt-config.ini").write_text(
                "[Data Storage]\n"
                "use_custom_storage = true\n"
                "use_virtual_sd = true\n"
                "nand_directory = CustomNand\n"
                "sdmc_directory = CustomSDMC\n",
                encoding="utf-8",
            )

            settings = azahar_directory_settings(
                str(emulator_dir / "azahar.exe"),
                '-f "%rom%"',
                str.split,
            )

        self.assertEqual(settings["user_root"], str((emulator_dir / "user").resolve()))
        self.assertEqual(settings["nand_root"], str((emulator_dir / "user" / "CustomNand").resolve()))
        self.assertEqual(settings["sdmc_root"], str((emulator_dir / "user" / "CustomSDMC").resolve()))
        self.assertEqual(settings["states_root"], str((emulator_dir / "user" / "states").resolve()))

    def test_azahar_save_path_overrides_include_sdmc_title_roots_and_states(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "azahar"
            config_dir = emulator_dir / "user" / "config"
            config_dir.mkdir(parents=True)
            (config_dir / "qt-config.ini").write_text(
                "[Data Storage]\n"
                "use_custom_storage = true\n"
                "use_virtual_sd = true\n"
                "nand_directory = CustomNand\n"
                "sdmc_directory = CustomSDMC\n",
                encoding="utf-8",
            )
            sdmc_title_root = (
                emulator_dir
                / "user"
                / "CustomSDMC"
                / "Nintendo 3DS"
                / "00000000000000000000000000000000"
                / "00000000000000000000000000000000"
                / "title"
                / "00040000"
            )
            nand_title_root = (
                emulator_dir
                / "user"
                / "CustomNand"
                / "00000000000000000000000000000000"
                / "title"
                / "00040010"
            )
            sdmc_title_root.mkdir(parents=True)
            nand_title_root.mkdir(parents=True)

            save_overrides = azahar_save_path_overrides(
                str(emulator_dir / "azahar.exe"),
                '-f "%rom%"',
                str.split,
            )
            state_overrides = azahar_state_path_overrides(
                str(emulator_dir / "azahar.exe"),
                '-f "%rom%"',
                str.split,
            )

        self.assertIn(str(sdmc_title_root.resolve()), save_overrides)
        self.assertIn(str(nand_title_root.resolve()), save_overrides)
        self.assertEqual(state_overrides, [str((emulator_dir / "user" / "states").resolve())])

    def test_eden_directory_settings_read_custom_storage_from_qt_config(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "eden"
            config_dir = emulator_dir / "user" / "config"
            config_dir.mkdir(parents=True)
            (config_dir / "qt-config.ini").write_text(
                "[Data Storage]\n"
                "use_custom_storage = true\n"
                "use_virtual_sd = true\n"
                "nand_directory = CustomNand\n"
                "sdmc_directory = CustomSDMC\n",
                encoding="utf-8",
            )

            settings = eden_directory_settings(
                str(emulator_dir / "eden.exe"),
                '-f -g "%rom%"',
                str.split,
            )

        self.assertEqual(settings["user_root"], str((emulator_dir / "user").resolve()))
        self.assertEqual(settings["nand_root"], str((emulator_dir / "user" / "CustomNand").resolve()))
        self.assertEqual(settings["sdmc_root"], str((emulator_dir / "user" / "CustomSDMC").resolve()))

    def test_eden_save_path_overrides_include_existing_user_save_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "eden"
            user_save_root = (
                emulator_dir
                / "user"
                / "nand"
                / "user"
                / "save"
                / "0000000000000000"
                / "00000000000000000000000000000001"
            )
            (user_save_root / "0100ABCD1234EF00").mkdir(parents=True)

            overrides = eden_save_path_overrides(
                str(emulator_dir / "eden.exe"),
                '-f -g "%rom%"',
                str.split,
            )

        self.assertEqual(overrides, [str(user_save_root.resolve())])

    def test_fbneo_directory_settings_read_paths_from_config_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "fbneo"
            config_dir = emulator_dir / "config"
            config_dir.mkdir(parents=True)
            (config_dir / "fbneo.ini").write_text(
                "szAppHiscorePath support/custom-hiscores\n"
                "szAppHDDPath support/custom-hdd\n"
                "szAppEEPROMPath saves/custom-games\n",
                encoding="utf-8",
            )

            settings = fbneo_directory_settings(
                str(emulator_dir / "fbneo.exe"),
                '"%rom%"',
                str.split,
            )

        self.assertEqual(settings["config_path"], str((config_dir / "fbneo.ini").resolve()))
        self.assertEqual(settings["hiscore_path"], str((emulator_dir / "support" / "custom-hiscores").resolve()))
        self.assertEqual(settings["hdd_path"], str((emulator_dir / "support" / "custom-hdd").resolve()))
        self.assertEqual(settings["eeprom_path"], str((emulator_dir / "saves" / "custom-games").resolve()))
        self.assertEqual(settings["memcard_path"], str((emulator_dir / "config" / "memcards").resolve()))
        self.assertEqual(settings["state_path"], str((emulator_dir / "savestates").resolve()))

    def test_fbneo_save_and_state_path_overrides_include_configured_and_default_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "fbneo"
            config_dir = emulator_dir / "config"
            config_dir.mkdir(parents=True)
            (config_dir / "fbneo.ini").write_text(
                "szAppHiscorePath support/custom-hiscores\n"
                "szAppEEPROMPath config/games\n",
                encoding="utf-8",
            )

            save_overrides = fbneo_save_path_overrides(
                str(emulator_dir / "fbneo.exe"),
                '"%rom%"',
                str.split,
            )
            state_overrides = fbneo_state_path_overrides(
                str(emulator_dir / "fbneo.exe"),
                '"%rom%"',
                str.split,
            )

        self.assertEqual(
            save_overrides,
            [
                str((emulator_dir / "config" / "games").resolve()),
                str((emulator_dir / "config" / "memcards").resolve()),
                str((emulator_dir / "support" / "custom-hiscores").resolve()),
                str((emulator_dir / "support" / "hdd").resolve()),
            ],
        )
        self.assertEqual(state_overrides, [str((emulator_dir / "savestates").resolve())])

    def test_mame_directory_settings_read_inipath_and_custom_output_directories(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "mame"
            emulator_dir.mkdir()
            ini_dir = emulator_dir / "custom-ini"
            ini_dir.mkdir()
            (ini_dir / "mame.ini").write_text(
                "nvram_directory saves/nvram-custom\n"
                "diff_directory saves/diff-custom\n"
                "state_directory states/custom\n",
                encoding="utf-8",
            )

            settings = mame_directory_settings(
                str(emulator_dir / "mame.exe"),
                '-inipath custom-ini "%rom%"',
                str.split,
            )

        self.assertEqual(settings["ini_path"], str((ini_dir / "mame.ini").resolve()))
        self.assertEqual(settings["nvram_directory"], str((emulator_dir / "saves" / "nvram-custom").resolve()))
        self.assertEqual(settings["diff_directory"], str((emulator_dir / "saves" / "diff-custom").resolve()))
        self.assertEqual(settings["state_directory"], str((emulator_dir / "states" / "custom").resolve()))
        self.assertEqual(settings["memcard_directory"], str((emulator_dir / "memcard").resolve()))

    def test_mame_save_and_state_path_overrides_include_cli_and_default_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "mame"
            emulator_dir.mkdir()

            save_overrides = mame_save_path_overrides(
                str(emulator_dir / "mame.exe"),
                '-nvram_directory custom-nvram -diff_directory custom-diff "%rom%"',
                str.split,
            )
            state_overrides = mame_state_path_overrides(
                str(emulator_dir / "mame.exe"),
                '-state_directory custom-sta "%rom%"',
                str.split,
            )

        self.assertEqual(
            save_overrides,
            [
                str((emulator_dir / "custom-nvram").resolve()),
                str((emulator_dir / "memcard").resolve()),
                str((emulator_dir / "custom-diff").resolve()),
            ],
        )
        self.assertEqual(state_overrides, [str((emulator_dir / "custom-sta").resolve())])

    def test_pico8_directory_settings_prefer_home_argument(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "pico8"
            emulator_dir.mkdir()
            user_root = Path(temp_dir) / "Pico Home"
            user_root.mkdir()
            (user_root / "config.txt").write_text("root_path carts\n", encoding="utf-8")

            settings = pico8_directory_settings(
                str(emulator_dir / "pico8.exe"),
                f'-home "{user_root}" -run "%rom%"',
                str.split,
            )

        self.assertEqual(settings["user_root"], str(user_root.resolve()))
        self.assertEqual(settings["config_path"], str((user_root / "config.txt").resolve()))
        self.assertEqual(settings["cdata_root"], str((user_root / "cdata").resolve()))

    def test_pico8_save_path_overrides_use_home_cdata_and_cstore_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "pico8"
            emulator_dir.mkdir()
            user_root = Path(temp_dir) / "Pico Home"
            cdata_root = user_root / "cdata"
            cstore_root = user_root / "cstore"
            cdata_root.mkdir(parents=True)
            cstore_root.mkdir(parents=True)

            overrides = pico8_save_path_overrides(
                str(emulator_dir / "pico8.exe"),
                f'-home "{user_root}" -run "%rom%"',
                str.split,
            )

        self.assertEqual(overrides, [str(cdata_root.resolve()), str(cstore_root.resolve())])

    def test_xemu_directory_settings_honor_config_path_and_relative_storage_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "xemu"
            emulator_dir.mkdir()
            config_dir = Path(temp_dir) / "xemu-config"
            config_dir.mkdir()
            storage_dir = config_dir / "storage"
            storage_dir.mkdir()
            config_path = config_dir / "xemu.toml"
            config_path.write_text(
                "[sys.files]\n"
                "hdd_path = \"storage/custom-hdd.qcow2\"\n"
                "eeprom_path = \"storage/custom-eeprom.bin\"\n",
                encoding="utf-8",
            )

            settings = xemu_directory_settings(
                str(emulator_dir / "xemu.exe"),
                f'-full-screen -config_path "{config_path}" -dvd_path "%rom%"',
                str.split,
            )

        self.assertEqual(settings["config_path"], str(config_path.resolve()))
        self.assertEqual(settings["base_path"], str(config_dir.resolve()))
        self.assertEqual(settings["hdd_path"], str((storage_dir / "custom-hdd.qcow2").resolve()))
        self.assertEqual(settings["eeprom_path"], str((storage_dir / "custom-eeprom.bin").resolve()))

    def test_xemu_save_path_overrides_include_portable_hdd_and_default_eeprom(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "xemu"
            emulator_dir.mkdir()
            portable_hdd = emulator_dir / "portable-storage" / "xbox_hdd.qcow2"
            portable_hdd.parent.mkdir(parents=True)
            portable_hdd.write_text("", encoding="utf-8")
            (emulator_dir / "xemu.toml").write_text(
                "[sys.files]\n"
                "hdd_path = \"portable-storage/xbox_hdd.qcow2\"\n",
                encoding="utf-8",
            )

            overrides = xemu_save_path_overrides(
                str(emulator_dir / "xemu.exe"),
                '-full-screen -dvd_path "%rom%"',
                str.split,
            )

        self.assertEqual(overrides[0], str(portable_hdd.resolve()))
        self.assertIn(str((emulator_dir / "eeprom.bin").resolve()), overrides)

    def test_xenia_directory_settings_read_portable_content_root_from_config(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "xenia-canary"
            emulator_dir.mkdir()
            (emulator_dir / "portable.txt").write_text("", encoding="utf-8")
            config_path = emulator_dir / "xenia-canary.config.toml"
            config_path.write_text(
                '[Storage]\ncontent_root = "content-custom"\ncache_root = "cache-host"\n',
                encoding="utf-8",
            )

            settings = xenia_directory_settings(
                str(emulator_dir / "xenia_canary.exe"),
                f'--config "{config_path}" "%rom%"',
                str.split,
            )

        self.assertEqual(settings["variant"], "canary")
        self.assertEqual(settings["config_path"], str(config_path.resolve()))
        self.assertEqual(settings["storage_root"], str(emulator_dir.resolve()))
        self.assertEqual(settings["content_root"], str((emulator_dir / "content-custom").resolve()))
        self.assertEqual(settings["cache_root"], str((emulator_dir / "cache-host").resolve()))

    def test_xenia_save_path_overrides_include_saved_game_headers_and_profile_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "xenia-canary"
            emulator_dir.mkdir()
            (emulator_dir / "portable.txt").write_text("", encoding="utf-8")
            content_root = emulator_dir / "content"
            save_root = content_root / "0000000000000001" / "4D5307E6"
            (save_root / "00000001" / "SaveSlotA").mkdir(parents=True)
            (save_root / "Headers" / "00000001").mkdir(parents=True)
            (save_root / "profile" / "PlayerOne").mkdir(parents=True)
            (save_root / "00000002" / "DLC").mkdir(parents=True)

            overrides = xenia_save_path_overrides(
                str(emulator_dir / "xenia_canary.exe"),
                '"%rom%"',
                str.split,
            )
            state_overrides = xenia_state_path_overrides(
                str(emulator_dir / "xenia_canary.exe"),
                '"%rom%"',
                str.split,
            )

        self.assertIn(str((save_root / "00000001").resolve()), overrides)
        self.assertIn(str((save_root / "Headers" / "00000001").resolve()), overrides)
        self.assertIn(str((save_root / "profile").resolve()), overrides)
        self.assertNotIn(str((save_root / "00000002").resolve()), overrides)
        self.assertEqual(state_overrides, [])

    def test_xenia_is_edge_variant_detects_appimage_filename(self) -> None:
        self.assertTrue(_is_edge_variant("xenia_edge_linux.AppImage"))
        self.assertTrue(_is_edge_variant("/path/to/xenia-edge"))
        self.assertFalse(_is_edge_variant("xenia_canary.exe"))
        self.assertFalse(_is_edge_variant("xenia.exe"))

    @patch("grid_launcher.emulator.xenia._default_user_storage_root")
    def test_xenia_directory_settings_edge_variant_uses_edge_config(
        self, mock_storage_root
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            mock_storage_root.return_value = Path(temp_dir) / "nonexistent-storage"
            edge_dir = Path(temp_dir) / "xenia-edge"
            edge_dir.mkdir()
            config_path = edge_dir / "xenia-edge.config.toml"
            config_path.write_text(
                '[Storage]\ncontent_root = "content-edge"\n',
                encoding="utf-8",
            )

            settings = xenia_directory_settings(
                str(edge_dir / "xenia_edge_linux.AppImage"),
                '"%rom%"',
                str.split,
            )

        self.assertEqual(settings["variant"], "edge")
        self.assertEqual(settings["config_path"], str(config_path.resolve()))

    def test_xenia_edge_profile_in_autoprofiles_json(self) -> None:
        autoprofiles_path = Path(__file__).resolve().parents[1] / "emulator-autoprofiles.json"
        profiles = json.loads(autoprofiles_path.read_text(encoding="utf-8"))

        edge_profile = next(
            profile
            for profile in profiles
            if profile.get("name") == "Xenia Edge (Xbox 360)"
        )

        self.assertIn("xenia_edge_linux.AppImage", edge_profile["match_tokens"])
        linux_override = edge_profile["source"]["platform_overrides"]["linux"]
        self.assertIn("xenia_edge_linux.AppImage", linux_override["asset_patterns"])

    def test_redream_directory_settings_detect_portable_data_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "redream"
            emulator_dir.mkdir()
            config_path = emulator_dir / "redream.cfg"
            config_path.write_text("gamedir = games\n", encoding="utf-8")

            settings = redream_directory_settings(
                str(emulator_dir / "redream.exe"),
                '"%rom%"',
                str.split,
            )

        self.assertEqual(settings["data_root"], str(emulator_dir.resolve()))
        self.assertEqual(settings["config_path"], str(config_path.resolve()))
        self.assertEqual(settings["portable"], "true")

    def test_redream_save_and_state_path_overrides_include_global_vmus_and_state_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            emulator_dir = Path(temp_dir) / "redream"
            emulator_dir.mkdir()
            for file_name in ("vmu0.bin", "vmu1.bin", "vmu2.bin", "vmu3.bin", "flash.bin"):
                (emulator_dir / file_name).write_text("", encoding="utf-8")
            (emulator_dir / "Sonic Adventure.0.sav").write_text("", encoding="utf-8")

            save_overrides = redream_save_path_overrides(
                str(emulator_dir / "redream.exe"),
                '"%rom%"',
                str.split,
            )
            state_overrides = redream_state_path_overrides(
                str(emulator_dir / "redream.exe"),
                '"%rom%"',
                str.split,
            )

        self.assertEqual(
            save_overrides,
            [
                str((emulator_dir / "vmu0.bin").resolve()),
                str((emulator_dir / "vmu1.bin").resolve()),
                str((emulator_dir / "vmu2.bin").resolve()),
                str((emulator_dir / "vmu3.bin").resolve()),
            ],
        )
        self.assertEqual(state_overrides, [str(emulator_dir.resolve())])

    def test_cloud_save_block_reason_for_original_xbox_allows_xemu_shared_media_sync(self) -> None:
        reason = cloud_save_block_reason_for_game(
            {"platform": "Xbox", "title": "Halo"},
            is_native_executable_platform=is_native_executable_platform,
            emulator_name="Xemu",
            is_xemu_emulator_name=lambda value: "xemu" in value.casefold(),
        )

        self.assertEqual(reason, "")

    def test_cloud_save_block_reason_does_not_disable_xbox_360_by_name_match(self) -> None:
        reason = cloud_save_block_reason_for_game(
            {"platform": "Xbox 360", "title": "Forza"},
            is_native_executable_platform=is_native_executable_platform,
            emulator_name="Xenia Canary",
            is_xemu_emulator_name=lambda value: "xemu" in value.casefold(),
        )

        self.assertEqual(reason, "")

    def test_cloud_save_block_reason_for_redream_saves_allows_shared_vmu_sync(self) -> None:
        reason = cloud_save_block_reason_for_game(
            {"platform": "Dreamcast", "title": "Sonic Adventure"},
            is_native_executable_platform=is_native_executable_platform,
            emulator_name="Redream",
            is_xemu_emulator_name=lambda value: "xemu" in value.casefold(),
            is_redream_emulator_name=lambda value: "redream" in value.casefold(),
            save_type="save",
        )

        self.assertEqual(reason, "")

    def test_cloud_save_block_reason_does_not_disable_redream_states(self) -> None:
        reason = cloud_save_block_reason_for_game(
            {"platform": "Dreamcast", "title": "Sonic Adventure"},
            is_native_executable_platform=is_native_executable_platform,
            emulator_name="Redream",
            is_xemu_emulator_name=lambda value: "xemu" in value.casefold(),
            is_redream_emulator_name=lambda value: "redream" in value.casefold(),
            save_type="state",
        )

        self.assertEqual(reason, "")

    def test_emulator_entry_matches_tokens_uses_executable_path_when_name_is_customized(self) -> None:
        emulator = {
            "name": "Stan's Wii U Emulator",
            "path": r"C:\Emulators\Cemu\cemu.exe",
        }

        self.assertTrue(emulator_entry_matches_tokens(emulator, {"cemu"}))
        self.assertFalse(emulator_entry_matches_tokens(emulator, {"rpcs3"}))

    def test_matching_platforms_for_emulator_keywords_handles_compact_platform_labels(self) -> None:
        matches = matching_platforms_for_emulator_keywords(
            ["Sony PlayStation4", "PlayStation4", "Xbox360"],
            ["playstation 4"],
        )

        self.assertEqual(matches, ["Sony PlayStation4", "PlayStation4"])

    def test_matching_platforms_for_emulator_keywords_keeps_numeric_guard(self) -> None:
        matches = matching_platforms_for_emulator_keywords(
            ["Xbox360"],
            ["xbox"],
        )

        self.assertEqual(matches, [])

    def test_matching_platforms_for_emulator_keywords_handles_wiiu_and_xbox360_tokens(self) -> None:
        wii_matches = matching_platforms_for_emulator_keywords(["WiiU"], ["wii u"])
        xbox_matches = matching_platforms_for_emulator_keywords(["Xbox360"], ["xbox 360"])

        self.assertEqual(wii_matches, ["WiiU"])
        self.assertEqual(xbox_matches, ["Xbox360"])

    def test_matching_platforms_for_emulator_keywords_blocks_psp_for_playstation_keyword(self) -> None:
        matches = matching_platforms_for_emulator_keywords(
            ["Playstation Portable"],
            ["playstation"],
        )
        self.assertEqual(matches, [])

    def test_matching_platforms_for_emulator_keywords_allows_manufacturer_prefix_for_playstation(self) -> None:
        matches = matching_platforms_for_emulator_keywords(
            ["Sony PlayStation", "Playstation Portable"],
            ["playstation"],
        )
        self.assertEqual(matches, ["Sony PlayStation"])

    def test_install_block_reason_for_ps4_clears_when_shadps4_is_available(self) -> None:
        assignable = ["Sony PlayStation4"]
        keywords = ["playstation 4", "ps4"]
        supported = matching_platforms_for_emulator_keywords(assignable, keywords)

        def available_emulator_name(platform: str) -> str:
            return "ShadPS4" if any(candidate.casefold() == platform.casefold() for candidate in supported) else ""

        reason = install_block_reason_for_game(
            {"platform": "Sony PlayStation4"},
            lambda _game: False,
            lambda _game: False,
            available_emulator_name,
        )

        self.assertEqual(reason, "")

    def test_load_emulator_autoprofiles_returns_empty_list_when_json_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            profiles = load_emulator_autoprofiles(
                None,
                Path(temp_dir),
                normalize_save_strategy_value,
                normalize_ignore_extension_value,
            )

        self.assertEqual(profiles, [])

    def test_load_emulator_autoprofiles_returns_empty_list_when_json_is_invalid(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            base_path = Path(temp_dir)
            (base_path / "emulator-autoprofiles.json").write_text("not valid json", encoding="utf-8")

            profiles = load_emulator_autoprofiles(
                None,
                base_path,
                normalize_save_strategy_value,
                normalize_ignore_extension_value,
            )

        self.assertEqual(profiles, [])

    def test_emulator_profile_for_game_prefers_exact_title_when_match_tokens_duplicate(self) -> None:
        profiles = [
            {
                "match_tokens": ["shadps4.exe"],
                "name": "ShadPS4 Core (Playstation 4)",
                "args": "--core \"%rom%\"",
                "platform_keywords": ["playstation 4", "ps4"],
            },
            {
                "match_tokens": ["shadps4.exe"],
                "name": "ShadPS4 Qt Launcher (Playstation 4)",
                "args": "\"%rom%\"",
                "platform_keywords": ["playstation 4", "ps4"],
            },
        ]

        selected_profile = emulator_profile_for_game(
            {"title": "ShadPS4 Qt Launcher (Playstation 4)"},
            r"C:\Emulators\ShadPS4\shadPS4.exe",
            profiles,
        )

        self.assertEqual(selected_profile["name"], "ShadPS4 Qt Launcher (Playstation 4)")
        self.assertEqual(selected_profile["args"], '"%rom%"')


class ManualEmulatorAutopopulateTests(unittest.TestCase):
    def test_defaults_manual_entry_from_matched_profile(self) -> None:
        helper = _resolve_autoconfig_helper(
            "defaults_for_manual_emulator_entry",
            "defaulted_manual_emulator_entry",
        )
        entry = {
            "name": "My Cemu",
            "path": r"C:\Emulators\Cemu\cemu.exe",
            "args": "",
            "save_strategy": "",
            "ignore_files": "",
            "ignore_extensions": "",
            "save_paths": "",
            "state_paths": "",
        }
        profile = {
            "name": "Cemu (Wii U)",
            "match_tokens": ["cemu.exe"],
            "args": '-g "%rom%" -f',
            "save_strategy": "folder",
            "ignore_files": ["debug.log"],
            "ignore_extensions": ["tmp"],
            "save_directories": ["mlc01/usr/save"],
            "state_directories": ["states"],
        }

        result = _call_with_supported_kwargs(
            helper,
            entry=entry,
            manual_entry=entry,
            emulator_entry=entry,
            profiles=[profile],
            autoprofiles=[profile],
            normalize_save_strategy_value=normalize_save_strategy_value,
            emulator_entry_matches_tokens=emulator_entry_matches_tokens,
        )
        defaulted_entry = _coerce_manual_entry_result(result)

        self.assertEqual(defaulted_entry.get("name"), "My Cemu")
        self.assertEqual(defaulted_entry.get("path"), r"C:\Emulators\Cemu\cemu.exe")
        self.assertEqual(defaulted_entry.get("args"), '-g "%rom%" -f')
        self.assertEqual(defaulted_entry.get("save_strategy"), "folder")
        self.assertEqual(defaulted_entry.get("ignore_files"), "debug.log")
        self.assertEqual(defaulted_entry.get("ignore_extensions"), "tmp")
        self.assertEqual(defaulted_entry.get("save_paths"), "mlc01/usr/save")
        self.assertEqual(defaulted_entry.get("state_paths"), "states")

    def test_manual_entry_defaults_do_not_overwrite_user_values(self) -> None:
        helper = _resolve_autoconfig_helper(
            "defaults_for_manual_emulator_entry",
            "defaulted_manual_emulator_entry",
        )
        entry = {
            "name": "My Cemu",
            "path": r"C:\Emulators\Cemu\cemu.exe",
            "args": '"%rom%" --my-custom-flag',
            "save_strategy": "single_file",
            "ignore_files": "my-ignore.txt",
            "ignore_extensions": "bak",
            "save_paths": r"portable\my-saves",
            "state_paths": r"portable\my-states",
        }
        profile = {
            "name": "Cemu (Wii U)",
            "match_tokens": ["cemu.exe"],
            "args": '-g "%rom%" -f',
            "save_strategy": "folder",
            "ignore_files": ["debug.log"],
            "ignore_extensions": ["tmp"],
            "save_directories": ["mlc01/usr/save"],
            "state_directories": ["states"],
        }

        result = _call_with_supported_kwargs(
            helper,
            entry=entry,
            manual_entry=entry,
            emulator_entry=entry,
            profiles=[profile],
            autoprofiles=[profile],
            normalize_save_strategy_value=normalize_save_strategy_value,
            emulator_entry_matches_tokens=emulator_entry_matches_tokens,
        )
        defaulted_entry = _coerce_manual_entry_result(result)

        self.assertEqual(defaulted_entry.get("args"), '"%rom%" --my-custom-flag')
        self.assertEqual(defaulted_entry.get("save_strategy"), "single_file")
        self.assertEqual(defaulted_entry.get("ignore_files"), "my-ignore.txt")
        self.assertEqual(defaulted_entry.get("ignore_extensions"), "bak")
        self.assertEqual(defaulted_entry.get("save_paths"), r"portable\my-saves")
        self.assertEqual(defaulted_entry.get("state_paths"), r"portable\my-states")

    def test_manual_entry_defaults_noop_when_profile_does_not_match(self) -> None:
        helper = _resolve_autoconfig_helper(
            "defaults_for_manual_emulator_entry",
            "defaulted_manual_emulator_entry",
        )
        entry = {
            "name": "My Cemu",
            "path": r"C:\Emulators\Cemu\cemu.exe",
            "args": "",
            "save_strategy": "",
            "ignore_files": "",
            "ignore_extensions": "",
            "save_paths": "",
            "state_paths": "",
        }
        non_matching_profile = {
            "name": "RPCS3 (Playstation 3)",
            "match_tokens": ["rpcs3.exe"],
            "args": '--no-gui "%RPCS3_GAMEID%:%ps3_gameid%"',
            "save_strategy": "folder",
            "save_directories": ["dev_hdd0/home/00000001/savedata"],
            "state_directories": [],
        }

        result = _call_with_supported_kwargs(
            helper,
            entry=entry,
            manual_entry=entry,
            emulator_entry=entry,
            profiles=[non_matching_profile],
            autoprofiles=[non_matching_profile],
            normalize_save_strategy_value=normalize_save_strategy_value,
            emulator_entry_matches_tokens=emulator_entry_matches_tokens,
        )
        defaulted_entry = _coerce_manual_entry_result(result)

        self.assertEqual(defaulted_entry, entry)

    def test_assign_default_platforms_and_retroarch_cores_from_profile_keywords(self) -> None:
        helper = _resolve_autoconfig_helper(
            "assign_default_platforms_for_emulator",
            "assign_default_platforms_for_manual_emulator",
        )
        defaults = {
            "Sony PlayStation": "",
            "Nintendo Wii U": "",
        }
        core_defaults = {}
        profile = {
            "platform_keywords": ["playstation", "ps1"],
            "all_platforms": False,
        }
        assignable_platforms = ["Sony PlayStation", "Nintendo Wii U"]

        result = _call_with_supported_kwargs(
            helper,
            emulator_name="RetroArch (Multi-System)",
            profile=profile,
            defaults=defaults,
            platform_defaults=defaults,
            core_defaults=core_defaults,
            assignable_platforms=assignable_platforms,
            matching_platforms_for_emulator_keywords=matching_platforms_for_emulator_keywords,
            is_retroarch_emulator_name=lambda value: "retroarch" in value.casefold(),
            installed_retroarch_cores_for_platform=(
                lambda platform, _emulator_name: ["pcsx_rearmed_libretro.dll"]
                if platform.casefold() == "sony playstation"
                else []
            ),
            default_assignable_server_platforms=lambda: assignable_platforms,
        )

        resolved_defaults = defaults
        resolved_core_defaults = core_defaults
        if isinstance(result, tuple):
            for item in result:
                if isinstance(item, dict) and "Sony PlayStation" in item:
                    if item.get("Sony PlayStation") == "RetroArch (Multi-System)":
                        resolved_defaults = item
                    if item.get("Sony PlayStation") == "pcsx_rearmed_libretro.dll":
                        resolved_core_defaults = item

        self.assertEqual(resolved_defaults.get("Sony PlayStation"), "RetroArch (Multi-System)")
        self.assertEqual(resolved_defaults.get("Nintendo Wii U"), "")
        self.assertEqual(resolved_core_defaults.get("Sony PlayStation"), "pcsx_rearmed_libretro.dll")


class ManualEmulatorAutofillSuggestionTests(unittest.TestCase):
    def test_autofill_suggestions_match_typed_prefix(self) -> None:
        helper = _resolve_autoconfig_helper(
            "manual_emulator_autofill_suggestions",
            "manual_emulator_autoprofile_suggestions",
            "autoprofile_suggestions_for_prefix",
            "suggest_autoprofile_names",
        )
        profiles = [
            {"name": "RetroArch (Multi-System)", "match_tokens": ["retroarch.exe"]},
            {"name": "RetroDeck", "match_tokens": ["retrodeck.exe"]},
            {"name": "Dolphin", "match_tokens": ["dolphin.exe"]},
        ]

        result = _call_with_supported_kwargs(
            helper,
            prefix="retro",
            typed_prefix="retro",
            query="retro",
            text="retro",
            profiles=profiles,
            autoprofiles=profiles,
        )
        suggestions = _coerce_suggestion_names(result)

        self.assertEqual(suggestions, ["RetroArch (Multi-System)", "RetroDeck"])

    def test_autofill_suggestions_return_empty_for_no_match(self) -> None:
        helper = _resolve_autoconfig_helper(
            "manual_emulator_autofill_suggestions",
            "manual_emulator_autoprofile_suggestions",
            "autoprofile_suggestions_for_prefix",
            "suggest_autoprofile_names",
        )
        profiles = [
            {"name": "RetroArch (Multi-System)", "match_tokens": ["retroarch.exe"]},
            {"name": "Dolphin", "match_tokens": ["dolphin.exe"]},
        ]

        result = _call_with_supported_kwargs(
            helper,
            prefix="xyz-no-match",
            typed_prefix="xyz-no-match",
            query="xyz-no-match",
            text="xyz-no-match",
            profiles=profiles,
            autoprofiles=profiles,
        )
        suggestions = _coerce_suggestion_names(result)

        self.assertEqual(suggestions, [])

    def test_builds_manual_entry_from_selected_autofill_suggestion(self) -> None:
        helper = _resolve_autoconfig_helper(
            "manual_entry_from_autofill_suggestion",
            "manual_emulator_entry_from_suggestion",
            "build_manual_emulator_entry_from_suggestion",
            "populated_manual_emulator_entry_from_suggestion",
        )
        profile = {
            "name": "RetroArch (Multi-System)",
            "match_tokens": ["retroarch.exe"],
            "args": '-L "%core%" "%rom%"',
            "save_strategy": "folder",
            "ignore_files": ["retroarch.log"],
            "ignore_extensions": ["tmp"],
            "save_directories": ["saves"],
            "state_directories": ["states"],
        }
        entry = {
            "name": "retro",
            "path": r"C:\Emulators\RetroArch\retroarch_custom.exe",
            "args": "%rom%",
            "save_strategy": "",
            "ignore_files": "",
            "ignore_extensions": "",
            "save_paths": "",
            "state_paths": "",
        }

        result = _call_with_supported_kwargs(
            helper,
            selected_suggestion="RetroArch (Multi-System)",
            suggestion_name="RetroArch (Multi-System)",
            profile_name="RetroArch (Multi-System)",
            selected_profile=profile,
            profile=profile,
            entry=entry,
            manual_entry=entry,
            emulator_entry=entry,
            profiles=[profile],
            autoprofiles=[profile],
            normalize_save_strategy_value=normalize_save_strategy_value,
        )
        defaulted_entry = _coerce_manual_entry_result(result)

        self.assertEqual(defaulted_entry.get("name"), "RetroArch (Multi-System)")
        self.assertEqual(defaulted_entry.get("path"), r"C:\Emulators\RetroArch\retroarch_custom.exe")
        self.assertEqual(defaulted_entry.get("args"), '-L "%core%" "%rom%"')
        self.assertEqual(defaulted_entry.get("save_strategy"), "folder")
        self.assertEqual(defaulted_entry.get("ignore_files"), "retroarch.log")
        self.assertEqual(defaulted_entry.get("ignore_extensions"), "tmp")
        self.assertEqual(defaulted_entry.get("save_paths"), "saves")
        self.assertEqual(defaulted_entry.get("state_paths"), "states")


if __name__ == "__main__":
    unittest.main()
