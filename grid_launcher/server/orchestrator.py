from __future__ import annotations

from typing import Any, Callable


def fetch_connection_payloads(api_get: Callable[[str, dict[str, Any] | None], Any]) -> tuple[Any, Any]:
    me_payload = api_get("/api/users/me", None)
    platforms_payload = api_get("/api/platforms", None)
    return me_payload, platforms_payload
