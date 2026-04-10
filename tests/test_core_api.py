from __future__ import annotations

import unittest
from io import BytesIO
from urllib.error import HTTPError

from rom_mate.core.api import build_auth_headers, build_binary_auth_headers, format_http_error_details


class CoreApiTests(unittest.TestCase):
    def test_binary_headers_use_octet_stream_accept(self) -> None:
        token = "abc123"
        json_headers = build_auth_headers(token)
        binary_headers = build_binary_auth_headers(token)

        self.assertEqual(json_headers["Accept"], "application/json")
        self.assertEqual(binary_headers["Accept"], "application/octet-stream, */*;q=0.9")
        self.assertEqual(binary_headers["Authorization"], "Bearer abc123")

    def test_format_http_error_details_truncates_body(self) -> None:
        body = b"error=" + (b"x" * 400)
        error = HTTPError("https://server.example/api/download", 500, "Server Error", None, BytesIO(body))

        detail = format_http_error_details(error, body_limit=80)

        self.assertIn("HTTP 500 Server Error", detail)
        self.assertIn("url=https://server.example/api/download", detail)
        self.assertIn("body=\"", detail)
        self.assertIn("...\"", detail)


if __name__ == "__main__":
    unittest.main()
