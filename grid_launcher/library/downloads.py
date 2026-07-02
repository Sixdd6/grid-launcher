from __future__ import annotations

from typing import Any


def percent_value(completed: int, total: int) -> int:
    if total <= 0:
        return 0
    return max(0, min(100, int((completed * 100) / total)))


def percent_text(completed: int, total: int) -> str:
    return f"{percent_value(completed, total)}%"


def stripped_text(value: Any, default: str = "") -> str:
    if not isinstance(value, str):
        return default
    stripped = value.strip()
    return stripped or default


def format_size(size_bytes: float) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    size = float(max(0.0, size_bytes))
    unit_index = 0
    while size >= 1024.0 and unit_index < len(units) - 1:
        size /= 1024.0
        unit_index += 1
    precision = 0 if unit_index == 0 else 1
    return f"{size:.{precision}f} {units[unit_index]}"


def download_entry_detail_text(entry: dict[str, Any]) -> str:
    status = str(entry.get("status", ""))
    downloaded = int(entry.get("downloaded_bytes", 0))
    total = int(entry.get("total_bytes", 0))
    speed_bps = float(entry.get("speed_bps", 0.0))
    install_processed = int(entry.get("install_processed_bytes", 0))
    install_total = int(entry.get("install_total_bytes", 0))
    if status == "queued":
        return "Queued"
    if status == "downloading":
        if total > 0:
            percent = percent_text(downloaded, total)
            return (
                f"Downloading {percent} • {format_size(downloaded)} / {format_size(total)}"
                f" • {format_size(speed_bps)}/s"
            )
        return f"Downloading • {format_size(downloaded)} • {format_size(speed_bps)}/s"
    if status == "installing":
        if install_total > 0:
            install_percent = percent_text(install_processed, install_total)
            return (
                f"Installing {install_percent} • {format_size(install_processed)}"
                f" / {format_size(install_total)}"
            )
        return "Installing..."
    if status == "cancelling":
        return "Cancelling..."
    if status == "completed":
        size_text = format_size(downloaded) if downloaded > 0 else "Unknown size"
        return f"Completed • {size_text}"
    if status == "failed":
        error_text = str(entry.get("error", "")).strip() or "Unknown error"
        return f"Failed • {error_text}"
    if status == "cancelled":
        return "Cancelled"
    return status.capitalize() or "Unknown"


def download_count_text(
    active_download_count: int,
    queued_count: int,
    install_finalize_in_progress: bool,
) -> str:
    has_active_downloads = active_download_count > 0
    active_suffix = "s" if active_download_count != 1 else ""
    if install_finalize_in_progress and not has_active_downloads:
        if queued_count > 0:
            queued_suffix = "s" if queued_count != 1 else ""
            return f"Installing 1 game ({queued_count} queued download{queued_suffix})"
        return "Installing 1 game"
    if queued_count > 0:
        queued_suffix = "s" if queued_count != 1 else ""
        return f"{active_download_count} active download{active_suffix} ({queued_count} queued download{queued_suffix})"
    return f"{active_download_count} active download{active_suffix}"


def download_progress_display(
    active_download_count: int,
    active_download_bytes: int,
    active_download_total: int,
    install_finalize_in_progress: bool,
    active_install_bytes: int,
    active_install_total: int,
    queued_count: int,
) -> tuple[int, int, int, str]:
    has_active_downloads = active_download_count > 0
    if has_active_downloads and active_download_total > 0:
        percent = percent_value(active_download_bytes, active_download_total)
        return 0, 100, percent, f"{percent}%"
    if has_active_downloads:
        return 0, 0, 0, "Downloading..."
    if install_finalize_in_progress:
        if active_install_total > 0:
            percent = int(percent_text(active_install_bytes, active_install_total).rstrip("%"))
            return 0, 100, percent, "Installing..."
        return 0, 0, 0, "Installing..."
    if queued_count > 0:
        return 0, 100, 0, "Queued"
    return 0, 100, 0, "0%"


def download_speed_text(
    active_download_speed_bps: float,
    install_finalize_in_progress: bool,
    active_download_count: int,
    active_install_bytes: int,
    active_install_total: int,
) -> str:
    has_active_downloads = active_download_count > 0
    if install_finalize_in_progress and not has_active_downloads:
        if active_install_total > 0:
            install_percent = percent_text(active_install_bytes, active_install_total)
            return f"Installing {install_percent}"
        return "Installing..."
    speed_text = format_size(active_download_speed_bps)
    return f"{speed_text}/s"


def normalized_download_entry_identity(game: dict[str, Any]) -> tuple[str, str]:
    title_value = game.get("title", "Game")
    platform_value = game.get("platform", "")
    title = stripped_text(title_value, "Game")
    platform = stripped_text(platform_value)
    return title, platform


def make_download_entry_data(
    entry_id: str,
    game: dict[str, Any],
    status: str,
    error: str = "",
) -> dict[str, Any]:
    title, platform = normalized_download_entry_identity(game)
    return {
        "id": entry_id,
        "game": dict(game),
        "title": title,
        "platform": platform,
        "status": status,
        "downloaded_bytes": 0,
        "total_bytes": 0,
        "speed_bps": 0.0,
        "install_processed_bytes": 0,
        "install_total_bytes": 0,
        "error": error.strip(),
    }


def apply_download_entry_status(entry: dict[str, Any], status: str, error: str = "") -> None:
    entry["status"] = status
    entry["error"] = error.strip()
    if status in ("completed", "failed", "cancelled"):
        entry["speed_bps"] = 0.0


def apply_download_entry_progress(
    entry: dict[str, Any],
    downloaded_bytes: int,
    total_bytes: int,
    speed_bps: float,
) -> None:
    entry["downloaded_bytes"] = max(0, downloaded_bytes)
    entry["total_bytes"] = max(0, total_bytes)
    entry["speed_bps"] = max(0.0, speed_bps)


def apply_download_entry_install_progress(entry: dict[str, Any], installed_bytes: int, total_bytes: int) -> None:
    entry["install_processed_bytes"] = max(0, installed_bytes)
    entry["install_total_bytes"] = max(0, total_bytes)


def find_download_entry(download_entries: list[dict[str, Any]], entry_id: str) -> dict[str, Any] | None:
    for entry in download_entries:
        if entry.get("id") == entry_id:
            return entry
    return None


def remove_download_entry(download_entries: list[dict[str, Any]], entry_id: str) -> list[dict[str, Any]]:
    return [entry for entry in download_entries if entry.get("id") != entry_id]


def retry_download_game(entry: dict[str, Any]) -> dict[str, Any] | None:
    status = str(entry.get("status", ""))
    if status not in ("failed", "cancelled"):
        return None
    game_value = entry.get("game")
    if not isinstance(game_value, dict):
        return None
    game_copy = dict(game_value)
    game_copy.pop("_download_entry_id", None)
    return game_copy


def download_entry_title(entry: dict[str, Any]) -> str:
    title = str(entry.get("title", "Game"))
    platform = str(entry.get("platform", "")).strip()
    return title if not platform else f"{title} ({platform})"


def download_entry_action_mode(status: str) -> str:
    if status in ("queued", "downloading", "cancelling"):
        return "cancel"
    if status == "installing":
        return "installing"
    if status in ("failed", "cancelled"):
        return "retry-dismiss"
    return "dismiss"


def display_download_entries(download_entries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return list(reversed(download_entries))
