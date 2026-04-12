from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from rom_mate.library.ps3_links import (
    _has_ps3_game_content,
    detected_ps3_game_id,
    ps3_game_id_from_paths,
    ps3_link_plan_for_extracted_dir,
)


class Ps3LinkPlanTests(unittest.TestCase):
    def _path_key(self, path: Path) -> str:
        return str(path).casefold()

    def test_game_id_dir_becomes_junction(self) -> None:
        # extracted/games/NPUB12345/ -> mkdir target/games/, junction target/games/NPUB12345/
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "games" / "NPUB12345").mkdir(parents=True)
            target = Path(tmp) / "emulator"
            target.mkdir()

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)
            link_types = {str(target_path.relative_to(target)): link_type for _, target_path, _, link_type in plan}

            self.assertEqual(link_types.get("games"), "mkdir")
            self.assertEqual(link_types.get(str(Path("games") / "NPUB12345")), "junction")

    def test_dev_hdd0_trophy_path(self) -> None:
        # Full trophy path -> mkdirs for intermediates, junction at NPUB12345
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            trophy_path = extracted / "dev_hdd0" / "home" / "00000001" / "trophy" / "NPUB12345"
            trophy_path.mkdir(parents=True)
            target = Path(tmp) / "emulator"
            target.mkdir()

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)
            link_types = {str(target_path.relative_to(target)): link_type for _, target_path, _, link_type in plan}

            for intermediate in [
                "dev_hdd0",
                str(Path("dev_hdd0/home")),
                str(Path("dev_hdd0/home/00000001")),
                str(Path("dev_hdd0/home/00000001/trophy")),
            ]:
                self.assertEqual(link_types.get(intermediate), "mkdir", f"{intermediate} should be mkdir")

            self.assertEqual(link_types.get(str(Path("dev_hdd0/home/00000001/trophy/NPUB12345"))), "junction")

    def test_no_game_id_falls_back_to_junction(self) -> None:
        # No game ID anywhere -> entire top-level dir linked as junction
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "games" / "somedir").mkdir(parents=True)
            target = Path(tmp) / "emulator"
            target.mkdir()

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)
            link_types = {str(target_path.relative_to(target)): link_type for _, target_path, _, link_type in plan}

            self.assertEqual(link_types.get("games"), "junction")

    def test_existing_real_dir_is_skipped_recurse(self) -> None:
        # target/games/ already exists as real dir -> no mkdir for it, junction for NPUB12345
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "games" / "NPUB12345").mkdir(parents=True)
            target = Path(tmp) / "emulator"
            (target / "games").mkdir(parents=True)

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)
            link_types = {str(target_path.relative_to(target)): link_type for _, target_path, _, link_type in plan}

            self.assertNotIn("games", link_types)
            self.assertEqual(link_types.get(str(Path("games") / "NPUB12345")), "junction")

    def test_two_games_same_folder(self) -> None:
        # Two game IDs under games/ -> mkdir games/, two junctions
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "games" / "NPUB12345").mkdir(parents=True)
            (extracted / "games" / "BLES01234").mkdir(parents=True)
            target = Path(tmp) / "emulator"
            target.mkdir()

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)
            link_types = {str(target_path.relative_to(target)): link_type for _, target_path, _, link_type in plan}

            self.assertEqual(link_types.get("games"), "mkdir")
            self.assertEqual(link_types.get(str(Path("games") / "NPUB12345")), "junction")
            self.assertEqual(link_types.get(str(Path("games") / "BLES01234")), "junction")

    def test_detected_ps3_game_id_prefers_ps3_game_content_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "NPWR00558" / "TROPHY").mkdir(parents=True)
            (extracted / "BLUS30336" / "PS3_GAME").mkdir(parents=True)

            detected_id = detected_ps3_game_id(extracted, [])

            self.assertEqual(detected_id, "BLUS30336")

    def test_detected_ps3_game_id_ignores_non_ps3_game_dirs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "NPWR00558" / "TROPHY").mkdir(parents=True)

            detected_id = detected_ps3_game_id(extracted, [])

            self.assertEqual(detected_id, "")

    def test_link_plan_sorts_game_content_dir_first(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "NPWR00558" / "TROPHY").mkdir(parents=True)
            (extracted / "BLUS30336" / "PS3_GAME").mkdir(parents=True)
            target = Path(tmp) / "emulator"
            target.mkdir()

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)

            self.assertTrue(plan)
            self.assertEqual(plan[0][1].name, "BLUS30336")

    def test_has_ps3_game_content_true_when_ps3_game_exists(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            candidate = Path(tmp) / "BLUS30336"
            (candidate / "PS3_GAME").mkdir(parents=True)

            self.assertTrue(_has_ps3_game_content(candidate))

    def test_has_ps3_game_content_false_when_no_ps3_game(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            candidate = Path(tmp) / "NPWR00558"
            (candidate / "TROPHY").mkdir(parents=True)

            self.assertFalse(_has_ps3_game_content(candidate))

    def test_trophy_dir_routes_to_trophy_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "NPWR00558" / "TROPHY").mkdir(parents=True)
            target = Path(tmp) / "emulator"
            target.mkdir()

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)

            trophy_target = target / "dev_hdd0" / "home" / "00000001" / "trophy" / "NPWR00558"
            self.assertTrue(any(target_path == trophy_target and link_type == "junction" for _, target_path, _, link_type in plan))

    def test_game_dir_routes_to_emulator_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            extracted = Path(tmp) / "extracted"
            (extracted / "BLUS30336" / "PS3_GAME").mkdir(parents=True)
            target = Path(tmp) / "emulator"
            target.mkdir()

            plan = ps3_link_plan_for_extracted_dir(extracted, target, self._path_key)

            expected_target = target / "BLUS30336"
            self.assertTrue(any(target_path == expected_target and link_type == "junction" for _, target_path, _, link_type in plan))
            self.assertFalse(any("dev_hdd0" in target_path.parts and target_path.name == "BLUS30336" for _, target_path, _, _ in plan))

    def test_game_id_from_paths_skips_npwr(self) -> None:
        paths = [Path("portable/dev_hdd0/home/00000001/trophy/NPWR00558")]

        self.assertEqual(ps3_game_id_from_paths(paths), "")

    def test_game_id_from_paths_returns_blus_skips_npwr(self) -> None:
        paths = [
            Path("portable/dev_hdd0/home/00000001/trophy/NPWR00558"),
            Path("portable/BLUS30336"),
        ]

        self.assertEqual(ps3_game_id_from_paths(paths), "BLUS30336")


if __name__ == "__main__":
    unittest.main()
