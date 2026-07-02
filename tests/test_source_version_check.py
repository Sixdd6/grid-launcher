from __future__ import annotations

import json
import unittest
from urllib.error import HTTPError, URLError
from unittest.mock import MagicMock, patch

from grid_launcher.background.workers import SourceVersionCheckWorker


class SourceVersionCheckWorkerTests(unittest.TestCase):
    def _make_json_response(self, payload: dict[str, str]) -> MagicMock:
        response = MagicMock()
        response.read.return_value = json.dumps(payload).encode("utf-8")
        response.__enter__.return_value = response
        response.__exit__.return_value = False
        return response

    def test_github_latest_resolves_tag(self) -> None:
        worker = SourceVersionCheckWorker(
            {
                "provider": "github",
                "owner": "testowner",
                "repo": "testrepo",
                "release_tag": "latest",
            },
            "v0.9.0",
        )
        results: list[dict[str, str]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        with patch(
            "grid_launcher.background.workers.urlopen",
            return_value=self._make_json_response({"tag_name": "v1.2.3"}),
        ):
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0], {"installed_tag": "v0.9.0", "available_tag": "v1.2.3", "error": ""})

    def test_github_specific_tag_uses_tags_endpoint(self) -> None:
        worker = SourceVersionCheckWorker(
            {
                "provider": "github",
                "owner": "testowner",
                "repo": "testrepo",
                "release_tag": "v1.0.0",
            },
            "v0.9.0",
        )
        results: list[dict[str, str]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        with patch(
            "grid_launcher.background.workers.urlopen",
            return_value=self._make_json_response({"tag_name": "v1.0.0"}),
        ) as mock_urlopen:
            worker.run()

        self.assertEqual(len(results), 1)
        request = mock_urlopen.call_args[0][0]
        full_url = request.full_url if hasattr(request, "full_url") else request.get_full_url()
        self.assertIn("/releases/tags/v1.0.0", full_url)

    def test_gitea_latest_resolves_tag(self) -> None:
        worker = SourceVersionCheckWorker(
            {
                "provider": "gitea",
                "base_url": "https://git.example.com",
                "owner": "o",
                "repo": "r",
            },
            "v0.9.0",
        )
        results: list[dict[str, str]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        with patch(
            "grid_launcher.background.workers.urlopen",
            return_value=self._make_json_response({"tag_name": "v1.2.3"}),
        ):
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0], {"installed_tag": "v0.9.0", "available_tag": "v1.2.3", "error": ""})

    def test_direct_provider_emits_direct_with_no_network(self) -> None:
        worker = SourceVersionCheckWorker(
            {
                "provider": "direct",
                "owner": "testowner",
                "repo": "testrepo",
                "download_url": "https://example.com/emulator.zip",
            },
            "v0.9.0",
        )
        results: list[dict[str, str]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        with patch("grid_launcher.background.workers.urlopen") as mock_urlopen:
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0], {"installed_tag": "v0.9.0", "available_tag": "direct", "error": ""})
        mock_urlopen.assert_not_called()

    def test_network_error_emits_error(self) -> None:
        worker = SourceVersionCheckWorker(
            {
                "provider": "github",
                "owner": "testowner",
                "repo": "testrepo",
                "release_tag": "latest",
            },
            "v0.9.0",
        )
        results: list[dict[str, str]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        with patch("grid_launcher.background.workers.urlopen", side_effect=URLError("timeout")):
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertTrue(results[0].get("error", ""))

    def test_http_error_emits_error(self) -> None:
        worker = SourceVersionCheckWorker(
            {
                "provider": "github",
                "owner": "testowner",
                "repo": "testrepo",
                "release_tag": "latest",
            },
            "v0.9.0",
        )
        results: list[dict[str, str]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        error = HTTPError(
            "https://api.github.com/repos/testowner/testrepo/releases/latest",
            403,
            "Forbidden",
            {},
            None,
        )
        with patch("grid_launcher.background.workers.urlopen", side_effect=error):
            worker.run()

        self.assertEqual(len(results), 1)
        self.assertTrue(results[0].get("error", ""))

    def test_unsupported_provider_emits_error(self) -> None:
        worker = SourceVersionCheckWorker(
            {
                "provider": "badprovider",
                "owner": "testowner",
                "repo": "testrepo",
            },
            "v0.9.0",
        )
        results: list[dict[str, str]] = []
        worker.finished.connect(lambda payload: results.append(payload))

        worker.run()

        self.assertEqual(len(results), 1)
        self.assertTrue(results[0].get("error", ""))


if __name__ == "__main__":
    unittest.main()
