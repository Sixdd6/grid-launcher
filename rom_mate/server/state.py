from __future__ import annotations

from typing import Any


def credentials_present(config: dict[str, Any]) -> bool:
    server_url = config.get("server_url", "")
    api_token = config.get("api_token", "")
    if not isinstance(server_url, str) or not isinstance(api_token, str):
        return False
    return bool(server_url.strip() and api_token.strip())


def server_base_url(config: dict[str, Any]) -> str:
    server_url = config.get("server_url", "")
    if not isinstance(server_url, str):
        return ""
    return server_url.strip().rstrip("/")


def account_status_text(config: dict[str, Any], is_connected: bool) -> str:
    username = config.get("username", "")
    if isinstance(username, str) and username.strip() and is_connected:
        return f"Logged in as: {username.strip()}"
    return "Offline"
