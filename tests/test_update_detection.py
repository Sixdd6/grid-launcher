from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path
from typing import Any

from grid_launcher.core.config import normalize_installed_games
from grid_launcher.library.install_registry import build_installed_game_record
from grid_launcher.library.update_detection import (
    game_has_server_update,
    game_server_updated_at,
    has_newer_server_rom_version,
    rom_file_name_version,
)
from grid_launcher.server.catalog import games_from_rom_items


class UpdateDetectionTests(unittest.TestCase):
    @staticmethod
    def _game_key(game: dict[str, str]) -> tuple[str, str]:
        return (
            game.get("title", "").strip().casefold(),
            game.get("platform", "").strip().casefold(),
        )

    def test_games_from_rom_items_maps_server_updated_at(self) -> None:
        games, _ = games_from_rom_items(
            [
                {
                    "id": 42,
                    "name": "Updated Game",
                    "platform_display_name": "PS2",
                    "updated_at": "2026-04-10T14:30:00Z",
                }
            ],
            "PS2",
            cover_url_from_payload=lambda payload: "",
            screenshot_urls_from_payload=lambda payload: [],
        )

        self.assertEqual(len(games), 1)
        self.assertEqual(games[0]["server_updated_at"], "2026-04-10T14:30:00Z")

    def test_build_installed_game_record_persists_server_updated_at(self) -> None:
        installed = build_installed_game_record(
            {
                "title": "Installed Game",
                "platform": "PS2",
                "server_updated_at": "2026-04-10T14:30:00Z",
            },
            Path("Y:/downloads/installed-game.zip"),
            resolved_cover_url="",
            cached_cover_path="",
        )

        self.assertEqual(installed["server_updated_at"], "2026-04-10T14:30:00Z")

    def test_build_installed_game_record_persists_ra_id(self) -> None:
        installed = build_installed_game_record(
            {
                "title": "Installed Game",
                "platform": "PS2",
                "ra_id": "12345",
            },
            Path("Y:/downloads/installed-game.zip"),
            resolved_cover_url="",
            cached_cover_path="",
        )

        self.assertEqual(installed["ra_id"], "12345")

    def test_normalize_installed_games_preserves_server_updated_at(self) -> None:
        normalized = normalize_installed_games(
            [
                {
                    "title": "Installed Game",
                    "platform": "PS2",
                    "server_updated_at": "2026-04-10T14:30:00Z",
                }
            ],
            self._game_key,
        )

        self.assertEqual(normalized[0]["server_updated_at"], "2026-04-10T14:30:00Z")

    def test_normalize_installed_games_preserves_ra_id(self) -> None:
        normalized = normalize_installed_games(
            [
                {
                    "title": "Installed Game",
                    "platform": "PS2",
                    "ra_id": "42",
                }
            ],
            self._game_key,
        )

        self.assertEqual(normalized[0]["ra_id"], "42")

    def test_normalize_installed_games_preserves_local_path(self) -> None:
        normalized = normalize_installed_games(
            [
                {
                    "title": "SNES Game",
                    "platform": "SNES",
                    "local_path": "/library/SNES/game.sfc",
                }
            ],
            self._game_key,
        )

        self.assertEqual(normalized[0]["local_path"], "/library/SNES/game.sfc")

    def test_game_has_server_update_true_when_server_timestamp_is_newer(self) -> None:
        installed = {
            "title": "Installed Game",
            "platform": "PS2",
            "server_updated_at": "2026-04-09T14:30:00Z",
        }
        server = {
            "title": "Installed Game",
            "platform": "PS2",
            "server_updated_at": "2026-04-10T14:30:00Z",
        }

        self.assertTrue(game_has_server_update(installed, server))

    def test_game_has_server_update_false_when_legacy_install_missing_timestamp(self) -> None:
        installed = {
            "title": "Legacy Install",
            "platform": "PS2",
        }
        server = {
            "title": "Legacy Install",
            "platform": "PS2",
            "server_updated_at": "2026-04-10T14:30:00Z",
        }

        self.assertFalse(game_has_server_update(installed, server))

    def test_game_has_server_update_false_for_emulators_platform(self) -> None:
        installed = {
            "title": "DuckStation",
            "platform": "Emulators",
            "server_updated_at": "2026-04-09T14:30:00Z",
        }
        server = {
            "title": "DuckStation",
            "platform": "Emulators",
            "server_updated_at": "2026-04-10T14:30:00Z",
        }

        self.assertFalse(game_has_server_update(installed, server))

    def test_game_server_updated_at_supports_fallback_keys(self) -> None:
        self.assertEqual(
            game_server_updated_at({"updated_at": "2026-04-10T14:30:00Z"}),
            "2026-04-10T14:30:00Z",
        )

    def test_rom_file_name_version_extracts_v_five_digits(self) -> None:
        self.assertEqual(
            rom_file_name_version("My Game (v00042).zip"),
            42,
        )

    def test_rom_file_name_version_extracts_semver_from_real_filename(self) -> None:
        self.assertEqual(
            rom_file_name_version("A Little to the Left (v3.6.0) (2022) (W_P).7z"),
            "3.6.0",
        )

    def test_rom_file_name_version_returns_none_without_matching_tag(self) -> None:
        self.assertIsNone(rom_file_name_version("My Game (v1234).zip"))
        self.assertIsNone(rom_file_name_version("My Game.zip"))

    def test_has_newer_server_rom_version_compares_numerically(self) -> None:
        self.assertTrue(
            has_newer_server_rom_version(
                "My Game (v00009).zip",
                "My Game (v00010).zip",
            )
        )
        self.assertFalse(
            has_newer_server_rom_version(
                "My Game (v00010).zip",
                "My Game (v00010).zip",
            )
        )

    def test_has_newer_server_rom_version_returns_false_when_missing_tags(self) -> None:
        self.assertFalse(
            has_newer_server_rom_version(
                "My Game.zip",
                "My Game (v00010).zip",
            )
        )

    def test_has_newer_server_rom_version_compares_dotted_semver_parts(self) -> None:
        self.assertTrue(
            has_newer_server_rom_version(
                "A Little to the Left (v3.5.9) (2022) (W_P).7z",
                "A Little to the Left (v3.6.0) (2022) (W_P).7z",
            )
        )
        self.assertFalse(
            has_newer_server_rom_version(
                "A Little to the Left (v3.6.0) (2022) (W_P).7z",
                "A Little to the Left (v3.5.9) (2022) (W_P).7z",
            )
        )
        self.assertFalse(
            has_newer_server_rom_version(
                "A Little to the Left (v3.6.0) (2022) (W_P).7z",
                "A Little to the Left (v3.6.0) (2022) (W_P).7z",
            )
        )
        self.assertTrue(
            has_newer_server_rom_version(
                "A Little to the Left (v3.6.0) (2022) (W_P).7z",
                "A Little to the Left (v3.6.0.1) (2022) (W_P).7z",
            )
        )

    def test_has_newer_server_rom_version_mixed_numeric_and_semver_is_false(self) -> None:
        self.assertFalse(
            has_newer_server_rom_version(
                "My Game (v01234).zip",
                "My Game (v3.6.0).zip",
            )
        )
        self.assertFalse(
            has_newer_server_rom_version(
                "My Game (v3.6.0).zip",
                "My Game (v01234).zip",
            )
        )

    def test_game_has_server_update_uses_windows_rom_file_version_when_available(self) -> None:
        installed = {
            "title": "Windows Game",
            "platform": "Windows",
            "rom_file_name": "Windows Game (v00009).zip",
        }
        server = {
            "title": "Windows Game",
            "platform": "Windows",
            "rom_file_name": "Windows Game (v00010).zip",
        }

        self.assertTrue(game_has_server_update(installed, server))

    def test_game_has_server_update_non_windows_missing_timestamps_stays_false(self) -> None:
        installed = {
            "title": "PS2 Game",
            "platform": "PS2",
            "rom_file_name": "PS2 Game (v00009).zip",
        }
        server = {
            "title": "PS2 Game",
            "platform": "PS2",
            "rom_file_name": "PS2 Game (v00010).zip",
        }

        self.assertFalse(game_has_server_update(installed, server))


class MainWindowUpdateFlowTests(unittest.TestCase):
    @staticmethod
    def _load_main_module() -> Any:
        module_path = Path(__file__).resolve().parents[1] / "grid-launcher.py"
        spec = importlib.util.spec_from_file_location("grid_launcher_main_for_update_tests", module_path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    class _RefreshStub:
        def __init__(self, module: Any) -> None:
            self.module = module
            self.library_games = [
                {
                    "title": "Updated Game",
                    "platform": "PS2",
                    "rom_id": "100",
                    "server_updated_at": "2026-04-09T00:00:00Z",
                },
                {
                    "title": "Legacy Install",
                    "platform": "PS2",
                    "rom_id": "200",
                },
                {
                    "title": "DuckStation",
                    "platform": "Emulators",
                    "rom_id": "300",
                    "server_updated_at": "2026-04-09T00:00:00Z",
                },
            ]
            self.server_games_by_platform = {
                "PS2": [
                    {
                        "title": "Updated Game",
                        "platform": "PS2",
                        "rom_id": "100",
                        "server_updated_at": "2026-04-10T00:00:00Z",
                    },
                    {
                        "title": "Legacy Install",
                        "platform": "PS2",
                        "rom_id": "200",
                        "server_updated_at": "2026-04-10T00:00:00Z",
                    },
                ],
                "Emulators": [
                    {
                        "title": "DuckStation",
                        "platform": "Emulators",
                        "rom_id": "300",
                        "server_updated_at": "2026-04-10T00:00:00Z",
                    }
                ],
            }
            self.installed_game_update_keys: set[tuple[str, str]] = set()
            self.current_details_game: dict[str, str] | None = None
            self.update_details_action_calls = 0

        def _update_details_action_buttons(self) -> None:
            self.update_details_action_calls += 1

        def _is_emulators_platform(self, game: dict[str, str]) -> bool:
            return game.get("platform", "").strip().casefold() == "emulators"

        def _game_key(self, game: dict[str, str]) -> tuple[str, str]:
            return (
                game.get("title", "").strip().casefold(),
                game.get("platform", "").strip().casefold(),
            )

        def _rom_id_key(self, game: dict[str, str]) -> str:
            return game.get("rom_id", "").strip().casefold()

        def _installed_game_record(self, game: dict[str, str]) -> dict[str, str] | None:
            target = self._game_key(game)
            for installed in self.library_games:
                if self._game_key(installed) == target:
                    return installed
            return None

        def _server_game_for_identity(self, game: dict[str, str], rom_id: str = "") -> dict[str, str] | None:
            return self.module.MainWindow._server_game_for_identity(self, game, rom_id)

        def _details_update_available_for_game(self, game: dict[str, str]) -> bool:
            return self.module.MainWindow._details_update_available_for_game(self, game)

    def test_refresh_installed_game_update_state_marks_only_supported_updates(self) -> None:
        module = self._load_main_module()
        window = self._RefreshStub(module)

        module.MainWindow._refresh_installed_game_update_state(window)

        self.assertEqual(
            window.installed_game_update_keys,
            {("updated game", "ps2")},
        )
        self.assertEqual(window.library_games[0].get("update_available"), "true")
        self.assertEqual(window.library_games[1].get("update_available"), "false")
        self.assertEqual(window.library_games[2].get("update_available"), "false")
        # _update_details_action_buttons is no longer called unconditionally; it only
        # fires a targeted update-button refresh when current_details_game is set.
        self.assertEqual(window.update_details_action_calls, 0)

    class _SecondaryActionStub:
        def __init__(self, module: Any, *, update_available: bool) -> None:
            self.module = module
            self.current_details_game = {
                "title": "Updated Game",
                "platform": "PS2",
                "rom_id": "100",
            }
            self.update_available = update_available
            self.started_install_payload: dict[str, str] | None = None
            self.uninstall_calls = 0

        def _is_game_installed(self, game: dict[str, str]) -> bool:
            return True

        def _resolve_rom_id_for_game(self, game: dict[str, str]) -> str:
            return "100"

        def _cache_rom_id_for_details_game(self, game: dict[str, str], rom_id: str) -> None:
            return None

        def _details_update_available_for_game(self, game: dict[str, str]) -> bool:
            return self.update_available

        def _server_game_for_identity(self, game: dict[str, str], rom_id: str = "") -> dict[str, str] | None:
            return {
                "title": "Updated Game",
                "platform": "PS2",
                "rom_id": "100",
                "server_updated_at": "2026-04-10T00:00:00Z",
            }

        def _hydrate_install_game_metadata(self, game: dict[str, str], rom_id: str) -> None:
            game["rom_id"] = rom_id

        def _resolved_rom_file_name_for_game(self, game: dict[str, str], rom_id: str) -> str:
            return "updated-game.zip"

        def _start_async_install(self, game: dict[str, str]) -> bool:
            self.started_install_payload = dict(game)
            return True

        def _uninstall_game(self, game: dict[str, str]) -> bool:
            self.uninstall_calls += 1
            return True

        def _update_details_action_buttons(self) -> None:
            return None

        def _game_key(self, game: dict[str, str]) -> tuple[str, str]:
            return (
                game.get("title", "").strip().casefold(),
                game.get("platform", "").strip().casefold(),
            )

        def _refresh_installed_game_update_state(self) -> None:
            return None

    def test_secondary_action_keeps_uninstall_behavior_when_update_available(self) -> None:
        module = self._load_main_module()
        window = self._SecondaryActionStub(module, update_available=True)

        module.MainWindow._perform_game_secondary_action(window)

        self.assertIsNone(window.started_install_payload)
        self.assertEqual(window.uninstall_calls, 1)

    def test_secondary_action_keeps_uninstall_behavior_without_update(self) -> None:
        module = self._load_main_module()
        window = self._SecondaryActionStub(module, update_available=False)

        module.MainWindow._perform_game_secondary_action(window)

        self.assertIsNone(window.started_install_payload)
        self.assertEqual(window.uninstall_calls, 1)

    def test_update_action_starts_update_install_when_update_available(self) -> None:
        module = self._load_main_module()
        window = self._SecondaryActionStub(module, update_available=True)

        module.MainWindow._perform_game_update_action(window)

        self.assertIsNotNone(window.started_install_payload)
        assert window.started_install_payload is not None
        self.assertEqual(window.started_install_payload.get("_install_mode"), "update")
        self.assertEqual(window.started_install_payload.get("rom_file_name"), "updated-game.zip")
        self.assertEqual(window.uninstall_calls, 0)

    def test_update_action_does_nothing_without_update(self) -> None:
        module = self._load_main_module()
        window = self._SecondaryActionStub(module, update_available=False)

        module.MainWindow._perform_game_update_action(window)

        self.assertIsNone(window.started_install_payload)
        self.assertEqual(window.uninstall_calls, 0)


if __name__ == "__main__":
    unittest.main()
