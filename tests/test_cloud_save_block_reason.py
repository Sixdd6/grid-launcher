import unittest

from rom_mate.emulator.selection import cloud_save_block_reason_for_game


def _is_retroarch(name: str) -> bool:
    return "retroarch" in name.lower()


def _is_not_retroarch(name: str) -> bool:
    return False


def _is_native(game: dict) -> bool:
    return game.get("platform", "") == "Windows"


class TestCloudSaveBlockReasonRetroarchCoreFlags(unittest.TestCase):
    """Tests for RetroArch core-flag gating in cloud_save_block_reason_for_game."""

    _SAFE_FLAGS = {
        "supports_save_states": True,
        "supports_saves": True,
        "cloud_sync_safe": True,
    }
    _MAME_FLAGS = {
        "supports_save_states": False,
        "supports_saves": False,
        "cloud_sync_safe": False,
    }
    _FBNEO_FLAGS = {
        "supports_save_states": True,
        "supports_saves": True,
        "cloud_sync_safe": False,
    }
    _GAME = {"platform": "Arcade", "title": "Street Fighter II"}

    def test_blocks_state_when_core_does_not_support_save_states(self):
        reason = cloud_save_block_reason_for_game(
            self._GAME,
            is_native_executable_platform=_is_native,
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags=self._MAME_FLAGS,
            save_type="state",
        )
        self.assertTrue(reason, "Expected a block reason for unsupported save states")

    def test_blocks_state_when_core_is_not_cloud_sync_safe(self):
        reason = cloud_save_block_reason_for_game(
            self._GAME,
            is_native_executable_platform=_is_native,
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags=self._FBNEO_FLAGS,
            save_type="state",
        )
        self.assertTrue(reason, "Expected a block reason for unsafe cloud sync")

    def test_does_not_block_state_for_safe_core(self):
        reason = cloud_save_block_reason_for_game(
            self._GAME,
            is_native_executable_platform=_is_native,
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags=self._SAFE_FLAGS,
            save_type="state",
        )
        self.assertEqual(reason, "", "Expected no block reason for safe core")

    def test_blocks_save_when_core_does_not_support_saves(self):
        reason = cloud_save_block_reason_for_game(
            self._GAME,
            is_native_executable_platform=_is_native,
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags=self._MAME_FLAGS,
            save_type="save",
        )
        self.assertTrue(reason, "Expected a block reason for unsupported saves")

    def test_does_not_block_save_for_safe_core(self):
        reason = cloud_save_block_reason_for_game(
            self._GAME,
            is_native_executable_platform=_is_native,
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags=self._SAFE_FLAGS,
            save_type="save",
        )
        self.assertEqual(reason, "", "Expected no block reason for safe core")

    def test_flag_check_skipped_when_not_retroarch_emulator(self):
        # Even with unsafe flags, a non-RetroArch emulator should not be blocked
        reason = cloud_save_block_reason_for_game(
            self._GAME,
            is_native_executable_platform=_is_native,
            emulator_name="Mesen",
            is_retroarch_emulator_name=_is_not_retroarch,
            retroarch_core_flags=self._MAME_FLAGS,
            save_type="state",
        )
        self.assertEqual(reason, "", "Non-RetroArch emulator should not be blocked by core flags")

    def test_flag_check_skipped_when_no_flags_passed(self):
        # retroarch_core_flags=None -> existing callers unaffected
        reason = cloud_save_block_reason_for_game(
            self._GAME,
            is_native_executable_platform=_is_native,
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags=None,
            save_type="state",
        )
        self.assertEqual(reason, "", "None flags should not trigger any block")

    def test_native_platform_still_blocked_regardless_of_core_flags(self):
        windows_game = {"platform": "Windows", "title": "Some Game"}
        reason = cloud_save_block_reason_for_game(
            windows_game,
            is_native_executable_platform=_is_native,
            emulator_name="RetroArch",
            is_retroarch_emulator_name=_is_retroarch,
            retroarch_core_flags=self._SAFE_FLAGS,
            save_type="save",
        )
        self.assertTrue(reason, "Native Windows platform should still be blocked")


if __name__ == "__main__":
    unittest.main()
