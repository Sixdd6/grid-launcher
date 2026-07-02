from __future__ import annotations

from .identity import game_key


def is_game_install_queued(game: dict[str, str], install_queue: list[dict[str, str]]) -> bool:
    target = game_key(game)
    return any(game_key(queued_game) == target for queued_game in install_queue)


def pending_install_key(
    install_in_progress: bool,
    install_finalize_in_progress: bool,
    install_pending_game: dict[str, str] | None,
    install_finalize_game: dict[str, str] | None,
) -> tuple[str, str] | None:
    if install_in_progress and install_pending_game is not None:
        return game_key(install_pending_game)
    if install_finalize_in_progress and install_finalize_game is not None:
        return game_key(install_finalize_game)
    return None


def queued_install_keys(install_queue: list[dict[str, str]]) -> set[tuple[str, str]]:
    return {game_key(queued_game) for queued_game in install_queue}


def filter_queue_by_download_entry_id(
    install_queue: list[dict[str, str]],
    entry_id: str,
) -> list[dict[str, str]]:
    return [queued_game for queued_game in install_queue if queued_game.get("_download_entry_id") != entry_id]


def download_entry_status_from_error(error: str) -> str:
    if isinstance(error, str) and error and "cancel" in error.lower():
        return "cancelled"
    if isinstance(error, str) and error:
        return "failed"
    return "completed"


def active_download_count_after_finish(active_download_count: int) -> int:
    return max(0, active_download_count - 1)


def should_reset_active_download_metrics(active_download_count: int) -> bool:
    return active_download_count == 0


def normalized_download_progress(
    downloaded_bytes: int | float,
    total_bytes: int | float,
    speed_bps: int | float,
) -> tuple[int, int, float]:
    downloaded = int(downloaded_bytes) if isinstance(downloaded_bytes, (int, float)) else 0
    total = int(total_bytes) if isinstance(total_bytes, (int, float)) else 0
    speed = float(speed_bps) if isinstance(speed_bps, (int, float)) else 0.0
    return max(0, downloaded), max(0, total), max(0.0, speed)


def normalized_transfer_progress(installed_bytes: int | float, total_bytes: int | float) -> tuple[int, int]:
    installed = int(installed_bytes) if isinstance(installed_bytes, (int, float)) else 0
    total = int(total_bytes) if isinstance(total_bytes, (int, float)) else 0
    return max(0, installed), max(0, total)


def can_start_next_queued_install(
    install_in_progress: bool,
    install_finalize_in_progress: bool,
    install_queue: list[dict[str, str]],
) -> bool:
    return not install_in_progress and not install_finalize_in_progress and bool(install_queue)
