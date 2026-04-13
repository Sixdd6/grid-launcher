from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from rom_mate.library.archive_preparation import prepare_installed_game_without_ui
from rom_mate.library.ps3_install import (
    ps3_classify_extracted_contents,
    ps3_game_id_from_text,
    ps3_game_id_from_paths,
    detected_ps3_game_id,
    ps3_route_extracted_contents,
)


class PS3InstallTests(unittest.TestCase):
    def test_prepare_installed_game_without_ui_routes_ps3_game_id_dir_to_vfs_dev_hdd0(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            archive_path = temp_path / "BLUS30336.7z"
            archive_path.write_bytes(b"placeholder")
            dev_hdd0_root = temp_path / "dev_hdd0"

            def fake_extract(archive: Path, destination: Path, install_progress_callback=None) -> None:
                del archive, install_progress_callback
                destination.mkdir(parents=True, exist_ok=True)
                game_dir = destination / "BLUS30336"
                (game_dir / "PS3_GAME").mkdir(parents=True)
                (game_dir / "PS3_GAME" / "PARAM.SFO").write_bytes(b"sfo")

            with patch(
                "rom_mate.library.archive_preparation.extract_archive_into_directory",
                side_effect=fake_extract,
            ):
                prepared, warning_text = prepare_installed_game_without_ui(
                    {"title": "Test PS3 Game", "platform": "PlayStation 3"},
                    archive_path,
                    cleanup_archive_on_success=False,
                    should_extract_archive_for_game=lambda game, path: True,
                    extract_archive_for_game=lambda game, path, cb: (_ for _ in ()).throw(
                        AssertionError("extract_archive_for_game should not be called for PS3")
                    ),
                    is_ps3_platform=lambda game: True,
                    ps3_dev_hdd0_root=lambda game: dev_hdd0_root,
                )

        self.assertIsNotNone(prepared)
        self.assertEqual(warning_text, "")
        assert prepared is not None
        expected_game_dir = str(dev_hdd0_root / "game" / "BLUS30336")
        self.assertEqual(prepared.get("extracted_dir"), expected_game_dir)
        self.assertEqual(prepared.get("extracted_path"), expected_game_dir)
        self.assertEqual(prepared.get("ps3_game_id"), "BLUS30336")

    def test_prepare_installed_game_without_ui_returns_error_when_no_dev_hdd0_configured(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            archive_path = Path(temp_dir) / "game.7z"
            archive_path.write_bytes(b"placeholder")

            def fake_extract(archive: Path, destination: Path, install_progress_callback=None) -> None:
                del archive, install_progress_callback
                destination.mkdir(parents=True, exist_ok=True)
                game_dir = destination / "BLUS30336"
                (game_dir / "PS3_GAME").mkdir(parents=True)
                (game_dir / "PS3_GAME" / "PARAM.SFO").write_bytes(b"sfo")

            with patch(
                "rom_mate.library.archive_preparation.extract_archive_into_directory",
                side_effect=fake_extract,
            ):
                prepared, warning_text = prepare_installed_game_without_ui(
                    {"title": "Test PS3 Game", "platform": "PlayStation 3"},
                    archive_path,
                    cleanup_archive_on_success=False,
                    should_extract_archive_for_game=lambda game, path: True,
                    extract_archive_for_game=lambda game, path, cb: (_ for _ in ()).throw(
                        AssertionError("extract_archive_for_game should not be called for PS3")
                    ),
                    is_ps3_platform=lambda game: True,
                    ps3_dev_hdd0_root=lambda game: None,
                )

        self.assertIsNone(prepared)
        self.assertIn("No PS3 VFS", warning_text)

    def test_prepare_installed_game_without_ui_returns_error_when_no_game_id_found(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            archive_path = temp_path / "game.7z"
            archive_path.write_bytes(b"placeholder")
            dev_hdd0_root = temp_path / "dev_hdd0"

            def fake_extract(archive: Path, destination: Path, install_progress_callback=None) -> None:
                del archive, install_progress_callback
                destination.mkdir(parents=True, exist_ok=True)
                (destination / "some_unknown_dir").mkdir()
                (destination / "some_unknown_dir" / "data.bin").write_bytes(b"data")

            with patch(
                "rom_mate.library.archive_preparation.extract_archive_into_directory",
                side_effect=fake_extract,
            ):
                prepared, warning_text = prepare_installed_game_without_ui(
                    {"title": "Test PS3 Game", "platform": "PlayStation 3"},
                    archive_path,
                    cleanup_archive_on_success=False,
                    should_extract_archive_for_game=lambda game, path: True,
                    extract_archive_for_game=lambda game, path, cb: (_ for _ in ()).throw(
                        AssertionError("extract_archive_for_game should not be called for PS3")
                    ),
                    is_ps3_platform=lambda game: True,
                    ps3_dev_hdd0_root=lambda game: dev_hdd0_root,
                )

        self.assertIsNone(prepared)
        self.assertIn("No PS3 game ID", warning_text)


class PS3GameIdHelperTests(unittest.TestCase):
    def test_ps3_game_id_from_text_extracts_id(self) -> None:
        self.assertEqual(ps3_game_id_from_text("BLUS30336"), "BLUS30336")
        self.assertEqual(ps3_game_id_from_text("game_NPUB31234_v2"), "NPUB31234")
        self.assertEqual(ps3_game_id_from_text(""), "")
        self.assertEqual(ps3_game_id_from_text("random_file.7z"), "")

    def test_ps3_game_id_from_paths_skips_npwr(self) -> None:
        paths = [Path("dev_hdd0/home/00000001/trophy/NPWR01234"), Path("dev_hdd0/game/BLUS30001")]
        self.assertEqual(ps3_game_id_from_paths(paths), "BLUS30001")

    def test_ps3_game_id_from_paths_returns_empty_for_no_match(self) -> None:
        self.assertEqual(ps3_game_id_from_paths([Path("unknown/file")]), "")

    def test_detected_ps3_game_id_finds_from_dir_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            game_dir = Path(tmp) / "BCUS98174"
            (game_dir / "PS3_GAME").mkdir(parents=True)
            result = detected_ps3_game_id(Path(tmp), [])
        self.assertEqual(result, "BCUS98174")


class PS3ClassifyTests(unittest.TestCase):
    def test_classifies_game_id_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            game_dir = Path(tmp) / "BLUS30336"
            (game_dir / "PS3_GAME").mkdir(parents=True)
            (game_dir / "PS3_GAME" / "PARAM.SFO").write_bytes(b"sfo")

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "game_id_dir")
        self.assertEqual(results[0][0].name, "BLUS30336")

    def test_classifies_trophy_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "NPWR12345").mkdir()

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "trophy_dir")

    def test_classifies_bare_disc_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "PS3_GAME").mkdir()

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "bare_disc_dir")

    def test_classifies_iso_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "game.iso").write_bytes(b"iso")

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "iso_file")

    def test_classifies_nested_hdd0_layout(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            game_dir = Path(tmp) / "dev_hdd0" / "game" / "NPUB31000"
            game_dir.mkdir(parents=True)

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "nested_hdd0_game")

    def test_classifies_nested_hdd0_home_only_layout(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            trophy_dir = Path(tmp) / "dev_hdd0" / "home" / "00000001" / "trophy" / "NPWR00042"
            trophy_dir.mkdir(parents=True)

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "nested_hdd0_game")

    def test_classifies_config_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_dir = Path(tmp) / "config"
            (config_dir / "custom_configs").mkdir(parents=True)
            (config_dir / "custom_configs" / "BLUS30336.yml").write_bytes(b"settings")

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "config_dir")
        self.assertEqual(results[0][0].name, "config")

    def test_classifies_unknown_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "readme.txt").write_bytes(b"text")

            results = ps3_classify_extracted_contents(Path(tmp))

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0][1], "unknown")

    def test_classifies_multiple_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            game_dir = Path(tmp) / "BLUS30336"
            (game_dir / "PS3_GAME").mkdir(parents=True)
            (Path(tmp) / "NPWR00001").mkdir()

            results = ps3_classify_extracted_contents(Path(tmp))

        classifications = {p.name: c for p, c in results}
        self.assertEqual(classifications["BLUS30336"], "game_id_dir")
        self.assertEqual(classifications["NPWR00001"], "trophy_dir")


class PS3RouteTests(unittest.TestCase):
    def test_routes_game_id_dir_to_dev_hdd0_game(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            game_dir = extracted / "BLUS30336"
            (game_dir / "PS3_GAME").mkdir(parents=True)
            (game_dir / "PS3_GAME" / "PARAM.SFO").write_bytes(b"sfo")
            dev_hdd0 = tmp_path / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

            self.assertEqual(game_id, "BLUS30336")
            self.assertEqual(len(installed_paths), 1)
            self.assertEqual(installed_paths[0], dev_hdd0 / "game" / "BLUS30336")
            self.assertTrue((dev_hdd0 / "game" / "BLUS30336" / "PS3_GAME" / "PARAM.SFO").exists())

    def test_routes_trophy_dir_to_dev_hdd0_trophy(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            trophy_dir = extracted / "NPWR00042"
            trophy_dir.mkdir(parents=True)
            (trophy_dir / "TROPHY.TRP").write_bytes(b"trp")
            dev_hdd0 = tmp_path / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

            self.assertEqual(len(installed_paths), 1)
            expected = dev_hdd0 / "home" / "00000001" / "trophy" / "NPWR00042"
            self.assertEqual(installed_paths[0], expected)
            self.assertTrue((expected / "TROPHY.TRP").exists())

    def test_routes_nested_hdd0_layout(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            nested_game = extracted / "dev_hdd0" / "game" / "NPUB30000"
            nested_game.mkdir(parents=True)
            (nested_game / "eboot.bin").write_bytes(b"bin")
            dev_hdd0 = tmp_path / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

            self.assertEqual(game_id, "NPUB30000")
            self.assertTrue((dev_hdd0 / "game" / "NPUB30000" / "eboot.bin").exists())

    def test_routes_nested_hdd0_home_trophy_and_exdata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            # game dir
            nested_game = extracted / "dev_hdd0" / "game" / "BLUS30336"
            nested_game.mkdir(parents=True)
            (nested_game / "eboot.bin").write_bytes(b"bin")
            # trophy
            trophy_dir = extracted / "dev_hdd0" / "home" / "00000001" / "trophy" / "NPWR00099"
            trophy_dir.mkdir(parents=True)
            (trophy_dir / "TROPHY.TRP").write_bytes(b"trp")
            # exdata (RAP files)
            exdata_dir = extracted / "dev_hdd0" / "home" / "00000001" / "exdata"
            exdata_dir.mkdir(parents=True)
            (exdata_dir / "EP0000-BLUS30336_00-GAME00000000.rap").write_bytes(b"rap")
            dev_hdd0 = tmp_path / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

            self.assertEqual(game_id, "BLUS30336")
            # Trophy dir should be tracked in installed_paths for cleanup
            expected_trophy = dev_hdd0 / "home" / "00000001" / "trophy" / "NPWR00099"
            self.assertIn(expected_trophy, installed_paths)
            self.assertTrue((expected_trophy / "TROPHY.TRP").exists())
            # RAP file should be installed under exdata
            expected_rap = dev_hdd0 / "home" / "00000001" / "exdata" / "EP0000-BLUS30336_00-GAME00000000.rap"
            self.assertTrue(expected_rap.exists())

    def test_routes_nested_hdd0_game_and_home_only_no_top_level_game_id_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            # Only dev_hdd0/ at root — no separate top-level BLUS##### dir
            nested_game = extracted / "dev_hdd0" / "game" / "NPUB31500"
            nested_game.mkdir(parents=True)
            (nested_game / "eboot.bin").write_bytes(b"bin")
            trophy_dir = extracted / "dev_hdd0" / "home" / "00000001" / "trophy" / "NPWR00200"
            trophy_dir.mkdir(parents=True)
            (trophy_dir / "TROPHY.TRP").write_bytes(b"trp")
            exdata_dir = extracted / "dev_hdd0" / "home" / "00000001" / "exdata"
            exdata_dir.mkdir(parents=True)
            (exdata_dir / "NPUB31500_00-KEY000000000.rap").write_bytes(b"rap")
            dev_hdd0 = tmp_path / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

            self.assertEqual(game_id, "NPUB31500")
            self.assertTrue((dev_hdd0 / "game" / "NPUB31500" / "eboot.bin").exists())
            expected_trophy = dev_hdd0 / "home" / "00000001" / "trophy" / "NPWR00200"
            self.assertIn(expected_trophy, installed_paths)
            self.assertTrue((expected_trophy / "TROPHY.TRP").exists())
            expected_rap = dev_hdd0 / "home" / "00000001" / "exdata" / "NPUB31500_00-KEY000000000.rap"
            self.assertTrue(expected_rap.exists())

    def test_routes_game_and_trophy_together(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            game_dir = extracted / "BCUS98174"
            (game_dir / "PS3_GAME").mkdir(parents=True)
            (game_dir / "PS3_GAME" / "PARAM.SFO").write_bytes(b"sfo")
            trophy_dir = extracted / "NPWR00099"
            trophy_dir.mkdir()
            (trophy_dir / "TROPHY.TRP").write_bytes(b"trp")
            dev_hdd0 = tmp_path / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

        self.assertEqual(game_id, "BCUS98174")
        self.assertEqual(len(installed_paths), 2)
        path_names = {p.name for p in installed_paths}
        self.assertIn("BCUS98174", path_names)
        self.assertIn("NPWR00099", path_names)

    def test_routes_config_dir_to_data_root_config(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            game_dir = extracted / "BLUS30336"
            (game_dir / "PS3_GAME").mkdir(parents=True)
            (game_dir / "PS3_GAME" / "PARAM.SFO").write_bytes(b"sfo")
            config_dir = extracted / "config"
            (config_dir / "custom_configs").mkdir(parents=True)
            (config_dir / "custom_configs" / "BLUS30336.yml").write_bytes(b"settings")
            # dev_hdd0 sits one level below data_root
            data_root = tmp_path / "rpcs3_data"
            dev_hdd0 = data_root / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

            self.assertEqual(game_id, "BLUS30336")
            expected_config = data_root / "config"
            self.assertIn(expected_config, installed_paths)
            self.assertTrue((expected_config / "custom_configs" / "BLUS30336.yml").exists())

    def test_returns_empty_game_id_for_only_unknown_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            extracted = tmp_path / "extracted"
            extracted.mkdir()
            (extracted / "readme.txt").write_bytes(b"x")
            dev_hdd0 = tmp_path / "dev_hdd0"

            game_id, installed_paths = ps3_route_extracted_contents(
                extracted, dev_hdd0, lambda iso, tmp_dir: tmp_dir
            )

        self.assertEqual(game_id, "")
        self.assertEqual(installed_paths, [])


if __name__ == "__main__":
    unittest.main()
