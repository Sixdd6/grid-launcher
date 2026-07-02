from __future__ import annotations

import unittest

from rom_mate.emulator.source import (
    EmulatorSourceResolutionError,
    normalize_emulator_source_metadata,
    resolve_emulator_source_release_asset,
)
from rom_mate.emulator.profiles import normalize_emulator_autoprofiles


def _asset(name: str, url: str, **overrides: object) -> dict[str, object]:
    data: dict[str, object] = {
        "name": name,
        "browser_download_url": url,
        "state": "uploaded",
    }
    data.update(overrides)
    return data


def _release(
    tag_name: str,
    assets: list[dict[str, object]],
    **overrides: object,
) -> dict[str, object]:
    data: dict[str, object] = {
        "tag_name": tag_name,
        "assets": assets,
        "draft": False,
        "prerelease": False,
    }
    data.update(overrides)
    return data


class EmulatorSourceResolveSuccessTests(unittest.TestCase):
    def test_resolve_github_latest_release_asset_with_include_and_exclude_patterns(self) -> None:
        source = {
            "provider": "github",
            "owner": "RPCS3",
            "repo": "rpcs3-binaries-win",
            "asset_patterns": ["*win*x64*.zip", "*.zip"],
            "asset_exclude_patterns": ["*symbols*"],
        }
        releases = [
            _release(
                "v0.0.2",
                [
                    _asset("rpcs3-win-x64-symbols.zip", "https://example.test/symbols.zip"),
                    _asset("rpcs3-win-x64.zip", "https://example.test/rpcs3.zip"),
                ],
            )
        ]

        resolved = resolve_emulator_source_release_asset(source, releases)

        self.assertEqual(resolved["provider"], "github")
        self.assertEqual(resolved["owner"], "RPCS3")
        self.assertEqual(resolved["repo"], "rpcs3-binaries-win")
        self.assertEqual(resolved["release_tag"], "v0.0.2")
        self.assertEqual(resolved["asset_name"], "rpcs3-win-x64.zip")
        self.assertEqual(resolved["download_url"], "https://example.test/rpcs3.zip")

    def test_resolve_github_specific_tag_release(self) -> None:
        source = {
            "provider": "github-release",
            "owner": "duckstation",
            "repo": "duckstation",
            "tag": "v1.2.0",
            "asset_patterns": ["*windows*.zip"],
        }
        releases = [
            _release("v1.3.0", [_asset("duckstation-windows.zip", "https://example.test/newer.zip")]),
            _release("v1.2.0", [_asset("duckstation-windows.zip", "https://example.test/target.zip")]),
        ]

        resolved = resolve_emulator_source_release_asset(source, releases)

        self.assertEqual(resolved["release_tag"], "v1.2.0")
        self.assertEqual(resolved["download_url"], "https://example.test/target.zip")

    def test_resolve_github_releases_payload_dictionary(self) -> None:
        source = {
            "provider": "github",
            "owner": "xenia",
            "repository": "xenia-canary",
            "asset_patterns": ["*.zip"],
            "asset_preferred_patterns": ["*canary*"],
        }
        payload = {
            "releases": [
                _release(
                    "v2.0.0",
                    [
                        _asset("xenia-nightly.zip", "https://example.test/nightly.zip"),
                        _asset("xenia-canary.zip", "https://example.test/canary.zip"),
                    ],
                )
            ]
        }

        resolved = resolve_emulator_source_release_asset(source, payload)

        self.assertEqual(resolved["asset_name"], "xenia-canary.zip")
        self.assertEqual(resolved["download_url"], "https://example.test/canary.zip")


class EmulatorSourceResolveFailureTests(unittest.TestCase):
    def test_unsupported_provider_raises_clear_error(self) -> None:
        source = {
            "provider": "itch",
            "owner": "dev",
            "repo": "builds",
        }

        with self.assertRaises(EmulatorSourceResolutionError) as raised:
            resolve_emulator_source_release_asset(source, [])

        self.assertIn("Unsupported source provider", str(raised.exception))

    def test_missing_required_owner_raises_clear_error(self) -> None:
        with self.assertRaises(EmulatorSourceResolutionError) as raised:
            normalize_emulator_source_metadata(
                {
                    "provider": "github",
                    "repo": "emu",
                }
            )

        self.assertIn("missing required field 'owner'", str(raised.exception))

    def test_prerelease_not_allowed_raises_when_no_stable_release_exists(self) -> None:
        source = {
            "provider": "github",
            "owner": "azahar",
            "repo": "azahar",
            "asset_patterns": ["*.zip"],
        }
        releases = [
            _release(
                "v0.1.0-beta",
                [_asset("azahar-windows.zip", "https://example.test/beta.zip")],
                prerelease=True,
            )
        ]

        with self.assertRaises(EmulatorSourceResolutionError) as raised:
            resolve_emulator_source_release_asset(source, releases)

        self.assertIn("No usable GitHub release was found", str(raised.exception))

    def test_no_matching_asset_raises_and_lists_available_asset_names(self) -> None:
        source = {
            "provider": "github",
            "owner": "retroarch",
            "repo": "retroarch",
            "asset_patterns": ["*windows*x64*.zip"],
        }
        releases = [
            _release(
                "v1.0.0",
                [
                    _asset("retroarch-linux.tar.gz", "https://example.test/linux.tar.gz"),
                    _asset("retroarch-macos.zip", "https://example.test/macos.zip"),
                ],
            )
        ]

        with self.assertRaises(EmulatorSourceResolutionError) as raised:
            resolve_emulator_source_release_asset(source, releases)

        message = str(raised.exception)
        self.assertIn("No release asset matched configured patterns", message)
        self.assertIn("retroarch-linux.tar.gz", message)
        self.assertIn("retroarch-macos.zip", message)

    def test_invalid_release_payload_shape_raises_clear_error(self) -> None:
        source = {
            "provider": "github",
            "owner": "eden",
            "repo": "eden",
        }

        with self.assertRaises(EmulatorSourceResolutionError) as raised:
            resolve_emulator_source_release_asset(source, {"unexpected": []})

        self.assertIn("must be a release object", str(raised.exception))


class GiteaProviderTests(unittest.TestCase):
    def test_normalize_gitea_provider_canonical_name(self):
        metadata = {
            "provider": "gitea",
            "base_url": "https://git.example.com",
            "owner": "my-org",
            "repo": "my-repo",
        }
        normalized = normalize_emulator_source_metadata(metadata)
        self.assertEqual(normalized["provider"], "gitea")
        self.assertEqual(normalized["base_url"], "https://git.example.com")

    def test_normalize_gitea_release_alias(self):
        metadata = {
            "provider": "gitea-release",
            "base_url": "https://git.example.com",
            "owner": "my-org",
            "repo": "my-repo",
        }
        normalized = normalize_emulator_source_metadata(metadata)
        self.assertEqual(normalized["provider"], "gitea")

    def test_normalize_gitea_missing_base_url_raises(self):
        metadata = {
            "provider": "gitea",
            "owner": "my-org",
            "repo": "my-repo",
        }
        with self.assertRaises(EmulatorSourceResolutionError) as ctx:
            normalize_emulator_source_metadata(metadata)
        self.assertIn("base_url", str(ctx.exception))

    def test_normalize_gitea_strips_trailing_slash_from_base_url(self):
        metadata = {
            "provider": "gitea",
            "base_url": "https://git.example.com/",
            "owner": "my-org",
            "repo": "my-repo",
        }
        normalized = normalize_emulator_source_metadata(metadata)
        self.assertFalse(normalized["base_url"].endswith("/"))

    def test_normalize_gitea_passes_through_platform_overrides(self):
        metadata = {
            "provider": "gitea",
            "base_url": "https://git.eden-emu.dev",
            "owner": "eden-emu",
            "repo": "eden",
            "asset_patterns": ["Eden-Windows-*-amd64-msvc-standard.zip"],
            "platform_overrides": {
                "linux": {
                    "asset_patterns": ["Eden-Linux-*-amd64-clang-pgo.AppImage"],
                    "launch_executable": "Eden-Linux-*-amd64-clang-pgo.AppImage",
                }
            },
        }
        normalized = normalize_emulator_source_metadata(metadata)
        self.assertEqual(
            normalized["platform_overrides"],
            {
                "linux": {
                    "asset_patterns": ["Eden-Linux-*-amd64-clang-pgo.AppImage"],
                    "launch_executable": "Eden-Linux-*-amd64-clang-pgo.AppImage",
                }
            },
        )

    def test_normalize_gitea_without_platform_overrides_omits_key(self):
        metadata = {
            "provider": "gitea",
            "base_url": "https://git.example.com",
            "owner": "my-org",
            "repo": "my-repo",
        }
        normalized = normalize_emulator_source_metadata(metadata)
        self.assertNotIn("platform_overrides", normalized)

    def test_resolve_gitea_latest_release_asset(self):
        source_metadata = {
            "provider": "gitea",
            "base_url": "https://git.example.com",
            "owner": "my-org",
            "repo": "my-repo",
            "release_tag": "latest",
            "asset_patterns": ["MyEmulator-Windows-*-amd64.zip"],
        }
        release_metadata = {
            "tag_name": "v1.2.3",
            "name": "Release v1.2.3",
            "draft": False,
            "prerelease": False,
            "assets": [
                {
                    "name": "MyEmulator-Windows-v1.2.3-amd64.zip",
                    "browser_download_url": "https://git.example.com/my-org/my-repo/releases/download/v1.2.3/MyEmulator-Windows-v1.2.3-amd64.zip",
                    "size": 1024,
                    "content_type": "application/zip",
                }
            ],
        }
        resolved = resolve_emulator_source_release_asset(source_metadata, release_metadata)
        self.assertEqual(resolved["provider"], "gitea")
        self.assertEqual(resolved["asset_name"], "MyEmulator-Windows-v1.2.3-amd64.zip")
        self.assertEqual(resolved["release_tag"], "v1.2.3")
        self.assertIn("amd64.zip", resolved["download_url"])


class EmulatorAutoprofileNormalizationTests(unittest.TestCase):
    def test_normalize_emulator_autoprofiles_preserves_source_metadata(self) -> None:
        profiles = [
            {
                "name": "DuckStation (Playstation 1)",
                "match_tokens": ["duckstation.exe"],
                "source": {
                    "provider": "github-release",
                    "owner": "stenzek",
                    "repo": "duckstation",
                    "asset_patterns": ["*windows*.zip"],
                },
            },
            {
                "name": "PCSX2 (Playstation 2)",
                "match_tokens": ["pcsx2-qt.exe"],
            },
        ]

        normalized = normalize_emulator_autoprofiles(
            profiles,
            lambda value: value,
            lambda value: value,
        )

        self.assertEqual(len(normalized), 2)
        self.assertIn("source", normalized[0])
        self.assertEqual(
            normalized[0]["source"],
            {
                "provider": "github-release",
                "owner": "stenzek",
                "repo": "duckstation",
                "asset_patterns": ["*windows*.zip"],
            },
        )
        self.assertNotIn("source", normalized[1])

    def test_normalize_emulator_autoprofiles_preserves_verified_source_windows_asset_patterns(self) -> None:
        profiles = [
            {
                "name": "Xenia Canary (Xbox 360)",
                "match_tokens": ["xenia_canary.exe"],
                "source": {
                    "provider": "github-release",
                    "owner": "xenia-canary",
                    "repo": "xenia-canary",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name": "xenia_canary_windows.zip",
                            "launch_executable": "xenia_canary.exe",
                        }
                    ],
                },
            },
            {
                "name": "Cemu (Wii U)",
                "match_tokens": ["cemu.exe"],
                "source": {
                    "provider": "github-release",
                    "owner": "cemu-project",
                    "repo": "Cemu",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name_regex": "^cemu-[0-9.]+-windows-x64\\.zip$",
                            "launch_executable": "cemu.exe",
                        }
                    ],
                },
            },
            {
                "name": "Xemu",
                "match_tokens": ["xemu.exe"],
                "source": {
                    "provider": "github-release",
                    "owner": "xemu-project",
                    "repo": "xemu",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name": "xemu-win-x86_64-release.zip",
                            "launch_executable": "xemu.exe",
                        },
                        {
                            "arch": "arm64",
                            "asset_name": "xemu-win-aarch64-release.zip",
                            "launch_executable": "xemu.exe",
                        },
                    ],
                },
            },
            {
                "name": "ShadPS4 (Playstation 4)",
                "match_tokens": ["shadPS4.exe"],
                "source": {
                    "provider": "github-release",
                    "owner": "shadps4-emu",
                    "repo": "shadPS4",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name_regex": "^shadps4-win64-sdl-[0-9.]+\\.zip$",
                            "launch_executable": "shadPS4.exe",
                        }
                    ],
                },
            },
            {
                "name": "Azahar (Nintendo 3DS)",
                "match_tokens": ["azahar.exe"],
                "source": {
                    "provider": "github-release",
                    "owner": "azahar-emu",
                    "repo": "azahar",
                    "release_tag": "latest",
                    "windows_assets": [
                        {
                            "arch": "x64",
                            "asset_name_regex": "^azahar-windows-msvc-[0-9.]+\\.zip$",
                            "launch_executable": "azahar.exe",
                        }
                    ],
                },
            },
        ]

        normalized = normalize_emulator_autoprofiles(
            profiles,
            lambda value: value,
            lambda value: value,
        )

        self.assertEqual(len(normalized), len(profiles))
        for index, profile in enumerate(profiles):
            self.assertEqual(normalized[index].get("source"), profile["source"])

    def test_normalize_emulator_autoprofiles_preserves_screenshot_directories(self) -> None:
        profiles = [
            {
                "name": "TestEmu",
                "match_tokens": ["test.exe"],
                "screenshot_directories": [
                    "screenshots",
                    "%LOCALAPPDATA%\\DuckStation\\screenshots",
                ],
            }
        ]

        normalized = normalize_emulator_autoprofiles(
            profiles,
            lambda value: value,
            lambda value: value,
        )

        self.assertEqual(len(normalized), 1)
        self.assertEqual(
            normalized[0]["screenshot_directories"],
            ["screenshots", "%LOCALAPPDATA%\\DuckStation\\screenshots"],
        )

    def test_normalize_emulator_autoprofiles_preserves_firmware_directories_strings(self) -> None:
        profiles = [
            {
                "name": "TestEmu",
                "match_tokens": ["test.exe"],
                "firmware_directories": ["system", "bios"],
            }
        ]

        normalized = normalize_emulator_autoprofiles(
            profiles,
            lambda value: value,
            lambda value: value,
        )

        self.assertEqual(len(normalized), 1)
        self.assertEqual(normalized[0]["firmware_directories"], ["system", "bios"])

    def test_normalize_emulator_autoprofiles_preserves_firmware_directories_dicts(self) -> None:
        directories = [
            {"path": "Sys/GC/JAP", "match": ["ntsc_j", "jap"]},
            {"path": "Sys/GC/USA", "match": ["ntsc", "usa"]},
        ]
        profiles = [
            {
                "name": "TestEmu",
                "match_tokens": ["test.exe"],
                "firmware_directories": directories,
            }
        ]

        normalized = normalize_emulator_autoprofiles(
            profiles,
            lambda value: value,
            lambda value: value,
        )

        self.assertEqual(len(normalized), 1)
        self.assertEqual(normalized[0]["firmware_directories"], directories)
        self.assertIsNot(normalized[0]["firmware_directories"][0], directories[0])
        self.assertIsNot(normalized[0]["firmware_directories"][1], directories[1])

    def test_normalize_emulator_autoprofiles_defaults_missing_directory_keys(self) -> None:
        profiles = [
            {
                "name": "TestEmu",
                "match_tokens": ["test.exe"],
            }
        ]

        normalized = normalize_emulator_autoprofiles(
            profiles,
            lambda value: value,
            lambda value: value,
        )

        self.assertEqual(len(normalized), 1)
        self.assertIn("screenshot_directories", normalized[0])
        self.assertIn("firmware_directories", normalized[0])
        self.assertEqual(normalized[0]["screenshot_directories"], [])
        self.assertEqual(normalized[0]["firmware_directories"], [])

    def test_normalize_emulator_autoprofiles_strips_invalid_firmware_directory_entries(self) -> None:
        profiles = [
            {
                "name": "TestEmu",
                "match_tokens": ["test.exe"],
                "firmware_directories": [
                    42,
                    "",
                    "  ",
                    None,
                    "bios",
                    {"path": "system", "match": ["all"]},
                ],
            }
        ]

        normalized = normalize_emulator_autoprofiles(
            profiles,
            lambda value: value,
            lambda value: value,
        )

        self.assertEqual(len(normalized), 1)
        self.assertEqual(
            normalized[0]["firmware_directories"],
            ["bios", {"path": "system", "match": ["all"]}],
        )


if __name__ == "__main__":
    unittest.main()
