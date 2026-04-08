from __future__ import annotations

from dataclasses import dataclass
from urllib.error import HTTPError, URLError


@dataclass(frozen=True)
class ConnectionFailure:
    status_text: str
    dialog_text: str
    token_expired: bool = False
    access_denied: bool = False


def classify_connection_failure(error: Exception | None) -> ConnectionFailure:
    if isinstance(error, HTTPError):
        if error.code == 401:
            return ConnectionFailure(
                status_text="Token expired",
                dialog_text="",
                token_expired=True,
            )
        if error.code == 403:
            access_denied_text = (
                "Access denied (403). Your account or token lacks required permissions. "
                "Create or use a token with API access, then update it in Settings."
            )
            return ConnectionFailure(
                status_text="Access denied (403)",
                dialog_text=access_denied_text,
                access_denied=True,
            )
        message = f"Connection failed ({error.code})"
        return ConnectionFailure(status_text=message, dialog_text=message)

    if isinstance(error, URLError):
        return ConnectionFailure(
            status_text="Connection failed (network error)",
            dialog_text="Connection failed (network error)",
        )

    return ConnectionFailure(status_text="Failed to connect", dialog_text="Failed to connect")
