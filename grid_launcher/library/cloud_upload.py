from __future__ import annotations

from pathlib import Path
from typing import Callable

from .cloud_transfer import uploaded_kind_label


def file_upload_jobs(files: list[Path], file_field: str) -> list[tuple[str, dict[str, Path]]]:
    return [(file_path.name, {file_field: file_path}) for file_path in files]


def directory_archive_upload_jobs(
    directories: list[Path],
    file_field: str,
    archive_builder: Callable[[Path], Path],
) -> tuple[list[tuple[str, dict[str, Path]]], list[Path]]:
    upload_jobs: list[tuple[str, dict[str, Path]]] = []
    temporary_archives: list[Path] = []

    for directory in directories:
        archive_path = archive_builder(directory)
        temporary_archives.append(archive_path)
        upload_jobs.append((directory.name, {file_field: archive_path}))

    return upload_jobs, temporary_archives


def no_matching_upload_message(save_type: str, *, is_ppsspp_state: bool = False) -> str:
    if is_ppsspp_state:
        return "No matching PPSSPP .ppst state files were found to upload."
    if save_type == "save":
        return "No matching save files or save folders were found to upload."
    return "No matching save states were found to upload."


def upload_completion_message(
    save_type: str,
    success_count: int,
    failed_files: list[str],
    retention_failed_ids: list[str],
    retention_limit: int,
) -> tuple[str, bool]:
    if failed_files and success_count == 0:
        return "Cloud upload failed for all matching files.", True

    kind_label = uploaded_kind_label(save_type)
    if failed_files:
        return f"Uploaded {success_count} {kind_label}. Failed: {', '.join(failed_files[:5])}", True

    if retention_failed_ids:
        return (
            f"Uploaded {success_count} {kind_label}. "
            f"Could not remove {len(retention_failed_ids)} older cloud saves for retention limit {retention_limit}.",
            True,
        )

    return f"Uploaded {success_count} {kind_label}.", False
