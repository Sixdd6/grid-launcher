from __future__ import annotations

import json
import mimetypes
import time
from io import UnsupportedOperation
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


def build_auth_headers(api_token: str) -> dict[str, str]:
    return {"Accept": "application/json", "Authorization": f"Bearer {api_token.strip()}"}


def build_binary_auth_headers(api_token: str) -> dict[str, str]:
    headers = build_auth_headers(api_token)
    headers["Accept"] = "application/octet-stream, */*;q=0.9"
    return headers


def format_http_error_details(error: HTTPError, *, body_limit: int = 240) -> str:
    status = int(getattr(error, "code", 0) or 0)
    reason = str(getattr(error, "reason", "") or "").strip()
    url = str(getattr(error, "url", "") or "").strip()

    title = f"HTTP {status}" if status > 0 else "HTTP error"
    if reason:
        title = f"{title} {reason}"

    parts = [title]
    if url:
        parts.append(f"url={url}")

    body_snippet = ""
    try:
        raw = error.read(body_limit + 1)
    except (OSError, ValueError, TypeError, UnsupportedOperation):
        raw = b""

    if isinstance(raw, bytes) and raw:
        decoded = raw.decode("utf-8", errors="replace")
        normalized = " ".join(decoded.split())
        truncated = normalized[:body_limit]
        if len(normalized) > body_limit:
            truncated = f"{truncated}..."
        if truncated:
            body_snippet = truncated

    if body_snippet:
        parts.append(f'body="{body_snippet}"')

    return " | ".join(parts)


def _build_url(base_url: str, path: str, params: dict[str, Any] | None = None) -> str:
    query = ""
    if params:
        query = urlencode(params, doseq=True)
    url = f"{base_url}{path}"
    if query:
        url = f"{url}?{query}"
    return url


def api_get_json(base_url: str, api_token: str, path: str, params: dict[str, Any] | None = None) -> Any:
    if not base_url:
        raise ValueError("Server URL is required")

    request = Request(_build_url(base_url, path, params), headers=build_auth_headers(api_token), method="GET")
    with urlopen(request, timeout=10) as response:
        raw = response.read().decode("utf-8")
    return json.loads(raw)


def api_get_bytes(base_url: str, api_token: str, path: str, params: dict[str, Any] | None = None) -> bytes:
    if not base_url:
        raise ValueError("Server URL is required")

    request = Request(_build_url(base_url, path, params), headers=build_binary_auth_headers(api_token), method="GET")
    with urlopen(request, timeout=60) as response:
        return response.read()


def multipart_payload(files: dict[str, Path]) -> tuple[str, bytes]:
    boundary = f"----RomMateBoundary{int(time.time() * 1000)}"
    body = bytearray()

    for field_name, file_path in files.items():
        if not file_path.exists() or not file_path.is_file():
            continue
        payload = file_path.read_bytes()
        mime_type = mimetypes.guess_type(file_path.name)[0] or "application/octet-stream"
        body.extend(f"--{boundary}\r\n".encode("utf-8"))
        body.extend(
            (
                f'Content-Disposition: form-data; name="{field_name}"; filename="{file_path.name}"\r\n'
                f"Content-Type: {mime_type}\r\n\r\n"
            ).encode("utf-8")
        )
        body.extend(payload)
        body.extend(b"\r\n")

    body.extend(f"--{boundary}--\r\n".encode("utf-8"))
    return f"multipart/form-data; boundary={boundary}", bytes(body)


def api_post_multipart_json(
    base_url: str,
    api_token: str,
    path: str,
    files: dict[str, Path],
    params: dict[str, Any] | None = None,
) -> Any:
    if not base_url:
        raise ValueError("Server URL is required")

    content_type, payload = multipart_payload(files)
    headers = build_auth_headers(api_token)
    headers["Content-Type"] = content_type

    request = Request(_build_url(base_url, path, params), headers=headers, method="POST", data=payload)
    with urlopen(request, timeout=60) as response:
        raw = response.read().decode("utf-8")
    return json.loads(raw)


def api_put_multipart_json(
    base_url: str,
    api_token: str,
    path: str,
    files: dict[str, Path],
    params: dict[str, Any] | None = None,
) -> Any:
    if not base_url:
        raise ValueError("Server URL is required")

    content_type, payload = multipart_payload(files)
    headers = build_auth_headers(api_token)
    headers["Content-Type"] = content_type

    request = Request(_build_url(base_url, path, params), headers=headers, method="PUT", data=payload)
    with urlopen(request, timeout=60) as response:
        raw = response.read().decode("utf-8")
    return json.loads(raw)


def api_post_json(
    base_url: str,
    api_token: str,
    path: str,
    payload: dict[str, Any],
    params: dict[str, Any] | None = None,
) -> Any:
    if not base_url:
        raise ValueError("Server URL is required")

    body = json.dumps(payload).encode("utf-8")
    headers = build_auth_headers(api_token)
    headers["Content-Type"] = "application/json"

    request = Request(_build_url(base_url, path, params), headers=headers, method="POST", data=body)
    with urlopen(request, timeout=60) as response:
        raw = response.read().decode("utf-8").strip()
    if not raw:
        return {}
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return {}
