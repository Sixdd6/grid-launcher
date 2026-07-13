import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from grid_launcher.library.archive_preparation import _flatten_single_subdirectory


class ArchiveFlatteningTests(unittest.TestCase):
    def test_single_subdirectory_is_flattened(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            target = Path(temp_dir)
            subdir = target / "subdir"
            subdir.mkdir()
            (subdir / "file1.txt").write_text("one", encoding="utf-8")
            (subdir / "file2.txt").write_text("two", encoding="utf-8")

            _flatten_single_subdirectory(target)

            self.assertFalse(subdir.exists())
            self.assertTrue((target / "file1.txt").exists())
            self.assertTrue((target / "file2.txt").exists())
            self.assertEqual((target / "file1.txt").read_text(encoding="utf-8"), "one")
            self.assertEqual((target / "file2.txt").read_text(encoding="utf-8"), "two")

    def test_multiple_top_level_items_are_not_flattened(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            target = Path(temp_dir)
            (target / "dir1").mkdir()
            (target / "dir2").mkdir()
            (target / "file.txt").write_text("data", encoding="utf-8")

            _flatten_single_subdirectory(target)

            self.assertTrue((target / "dir1").is_dir())
            self.assertTrue((target / "dir2").is_dir())
            self.assertTrue((target / "file.txt").exists())

    def test_single_file_is_not_flattened(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            target = Path(temp_dir)
            (target / "file.txt").write_text("data", encoding="utf-8")

            _flatten_single_subdirectory(target)

            self.assertEqual([entry.name for entry in target.iterdir()], ["file.txt"])
            self.assertTrue((target / "file.txt").exists())

    def test_empty_directory_is_not_flattened(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            target = Path(temp_dir)

            _flatten_single_subdirectory(target)

            self.assertEqual(list(target.iterdir()), [])

    def test_nested_structure_is_preserved(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            target = Path(temp_dir)
            deep = target / "subdir" / "nested" / "deep"
            deep.mkdir(parents=True)
            (deep / "file.txt").write_text("deep", encoding="utf-8")

            _flatten_single_subdirectory(target)

            self.assertFalse((target / "subdir").exists())
            moved_file = target / "nested" / "deep" / "file.txt"
            self.assertTrue(moved_file.exists())
            self.assertEqual(moved_file.read_text(encoding="utf-8"), "deep")


if __name__ == "__main__":
    unittest.main()
