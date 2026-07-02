import json
import unittest
from pathlib import Path

OPENAPI_PATH = Path(__file__).resolve().parent.parent / "openapi.json"


class OpenApiContractTests(unittest.TestCase):
    """Static contract checks against the checked-in openapi.json.

    These tests do not hit a live server. They only guard against a future
    openapi.json spec bump silently removing fields/params that
    GRID Launcher's code depends on.
    """

    @classmethod
    def setUpClass(cls) -> None:
        with OPENAPI_PATH.open(encoding="utf-8") as f:
            cls.spec = json.load(f)

    def _resolve_param_name(self, param: dict) -> str:
        if "name" in param:
            return param["name"]
        ref = param["$ref"]
        ref_name = ref.split("/")[-1]
        resolved = self.spec["components"]["parameters"][ref_name]
        return resolved["name"]

    def _assert_schema_has_properties(self, schema_name: str, expected_properties: set[str]) -> None:
        schema = self.spec["components"]["schemas"][schema_name]
        properties = set(schema["properties"].keys())
        missing = expected_properties - properties
        self.assertFalse(
            missing,
            f"{schema_name} is missing expected properties: {sorted(missing)}",
        )

    def test_roms_get_endpoint_has_expected_parameters(self) -> None:
        parameters = self.spec["paths"]["/api/roms"]["get"]["parameters"]
        param_names = {self._resolve_param_name(p) for p in parameters}
        expected = {
            "platform_ids",
            "genres",
            "order_by",
            "order_dir",
            "limit",
            "offset",
            "with_char_index",
            "with_filter_values",
        }
        missing = expected - param_names
        self.assertFalse(
            missing,
            f"/api/roms GET is missing expected parameters: {sorted(missing)}",
        )

    def test_simple_rom_schema_has_expected_properties(self) -> None:
        self._assert_schema_has_properties(
            "SimpleRomSchema",
            {
                "platform_display_name",
                "platform_slug",
                "platform_fs_slug",
                "fs_name",
                "fs_extension",
                "files",
                "updated_at",
                "ra_id",
            },
        )

    def test_detailed_rom_schema_has_expected_properties(self) -> None:
        self._assert_schema_has_properties(
            "DetailedRomSchema",
            {
                "platform_display_name",
                "platform_slug",
                "platform_fs_slug",
                "fs_name",
                "fs_extension",
                "files",
                "updated_at",
                "ra_id",
            },
        )

    def test_platform_schema_has_expected_properties(self) -> None:
        self._assert_schema_has_properties(
            "PlatformSchema",
            {
                "display_name",
                "name",
                "slug",
                "rom_count",
                "url_logo",
            },
        )

    def test_save_schema_has_expected_properties(self) -> None:
        self._assert_schema_has_properties(
            "SaveSchema",
            {
                "file_name",
                "download_path",
                "id",
                "rom_id",
            },
        )

    def test_state_schema_has_expected_properties(self) -> None:
        self._assert_schema_has_properties(
            "StateSchema",
            {
                "file_name",
                "download_path",
                "id",
                "rom_id",
            },
        )

    def test_firmware_schema_has_expected_properties(self) -> None:
        self._assert_schema_has_properties(
            "FirmwareSchema",
            {
                "id",
                "file_name",
            },
        )


if __name__ == "__main__":
    unittest.main()
