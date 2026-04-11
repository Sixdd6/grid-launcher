"""Helpers for fetching, downloading, and installing platform firmware files."""

from __future__ import annotations

import io
import shutil
import tempfile
import zipfile
from pathlib import Path


def fetch_platform_firmware(api_get_json_fn, platform_id: int) -> list[dict]:
    firmware = api_get_json_fn("/api/firmware", {"platform_id": platform_id})
    if isinstance(firmware, list):
        return firmware
    return []


def download_firmware_bytes(api_get_bytes_fn, firmware_id: int, file_name: str) -> bytes:
    return api_get_bytes_fn(f"/api/firmware/{firmware_id}/content/{file_name}")


def resolve_firmware_targets(file_name: str, target_dirs: list) -> list[Path]:
    """Return the subset of target_dirs that should receive this firmware file.

    Each entry in target_dirs is either:
    - A plain Path (accepts all firmware files)
    - A tuple (Path, list[str]) where the file is accepted only if any keyword
      appears as a substring of the lowercase filename

    For routed (tuple) entries, first-match-wins: once a tuple entry matches,
    remaining tuple entries are skipped. Plain Path entries always pass through.
    """
    lower_name = file_name.lower()
    result: list[Path] = []
    routed_match_found = False

    for entry in target_dirs:
        if isinstance(entry, tuple):
            if routed_match_found:
                continue
            path, keywords = entry
            if any(kw in lower_name for kw in keywords):
                result.append(path)
                routed_match_found = True
        else:
            result.append(entry)

    return result


def install_platform_firmware(
    api_get_json_fn,
    api_get_bytes_fn,
    platform_id: int,
    target_dirs: list,
    *,
    skip_existing: bool = True,
) -> list[str]:
    if not target_dirs:
        return []

    try:
        firmware_records = fetch_platform_firmware(api_get_json_fn, platform_id)
    except Exception as error:
        return [f"Firmware fetch failed for platform {platform_id}: {error}"]

    if not firmware_records:
        return []

    warnings: list[str] = []

    for record in firmware_records:
        firmware_id = record.get("id")
        file_name = record.get("file_name", "")
        if not isinstance(firmware_id, int) or not file_name:
            continue

        applicable_dirs = resolve_firmware_targets(file_name, target_dirs)
        if not applicable_dirs:
            continue

        try:
            data = download_firmware_bytes(api_get_bytes_fn, firmware_id, file_name)
        except Exception as error:
            warnings.append(f"Failed to download firmware {file_name}: {error}")
            continue

        for target_dir in applicable_dirs:
            try:
                target_dir.mkdir(parents=True, exist_ok=True)
            except OSError as error:
                warnings.append(f"Could not create firmware directory {target_dir}: {error}")
                continue

            dest_path = target_dir / file_name

            lower_name = file_name.lower()

            if lower_name.endswith(".7z") or lower_name.endswith(".rar"):
                try:
                    from rom_mate.library.archive_preparation import extract_archive_into_directory

                    with tempfile.NamedTemporaryFile(suffix=Path(file_name).suffix, delete=False) as tmp:
                        tmp_path = Path(tmp.name)
                        tmp.write(data)
                    try:
                        with tempfile.TemporaryDirectory() as staging:
                            staging_dir = Path(staging)
                            extract_archive_into_directory(tmp_path, staging_dir)
                            for extracted_file in staging_dir.rglob("*"):
                                if not extracted_file.is_file():
                                    continue
                                if "__MACOSX" in extracted_file.parts:
                                    continue
                                if extracted_file.name == ".DS_Store":
                                    continue
                                member_dest = target_dir / extracted_file.name
                                if skip_existing and member_dest.exists():
                                    continue
                                shutil.copy2(extracted_file, member_dest)
                    finally:
                        try:
                            tmp_path.unlink()
                        except OSError:
                            pass
                except Exception as error:
                    warnings.append(f"Failed to extract firmware archive {file_name}: {error}")
                    continue

            elif lower_name.endswith(".zip") or zipfile.is_zipfile(io.BytesIO(data)):
                try:
                    with zipfile.ZipFile(io.BytesIO(data)) as zf:
                        for member in zf.namelist():
                            if member.endswith("/") or member.startswith("__MACOSX"):
                                continue
                            extracted_name = Path(member).name
                            if not extracted_name:
                                continue
                            member_dest = target_dir / extracted_name
                            if skip_existing and member_dest.exists():
                                continue
                            member_dest.write_bytes(zf.read(member))
                except (zipfile.BadZipFile, OSError, KeyError) as error:
                    warnings.append(f"Failed to extract firmware archive {file_name}: {error}")
                    continue
            else:
                if skip_existing and dest_path.exists():
                    continue
                try:
                    dest_path.write_bytes(data)
                except OSError as error:
                    warnings.append(f"Failed to write firmware {file_name} to {dest_path}: {error}")
                    continue

    return warnings
