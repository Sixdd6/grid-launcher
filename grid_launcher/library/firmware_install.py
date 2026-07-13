"""Helpers for fetching, downloading, and installing platform firmware files."""

from __future__ import annotations

import io
import logging
import shutil
import ssl
import tempfile
import time
import urllib.request
import zipfile
from collections.abc import Callable
from pathlib import Path


_logger = logging.getLogger(__name__)

# Sony's PS3 firmware update manifest.  The manifest contains the CDN URL for
# the latest PS3UPDAT.PUP file and is fetched to resolve the current firmware
# download URL without hard-coding a version-specific path.
PS3_FIRMWARE_MANIFEST_URL = (
    "https://fus01.ps3.update.playstation.net/update/ps3/list/us/ps3-updatelist.txt"
)


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
    """
    lower_name = file_name.lower()
    result: list[Path] = []

    for entry in target_dirs:
        if isinstance(entry, tuple):
            path, keywords = entry
            if any(kw in lower_name for kw in keywords):
                result.append(path)
        else:
            result.append(entry)

    return result


def should_keep_zip_archive(file_name: str, target_dirs: list, applicable_dirs: list[Path]) -> bool:
    """Return whether this .zip should be written as-is instead of extracted.

    A .zip is preserved only when it was routed through at least one tuple entry
    whose keyword list contains the exact filename (case-insensitive).
    """
    lower_name = file_name.lower()
    applicable_set = set(applicable_dirs)

    for entry in target_dirs:
        if not isinstance(entry, tuple):
            continue
        path, keywords = entry
        if path not in applicable_set:
            continue

        lowered_keywords = [kw.lower() for kw in keywords]
        if any(kw in lower_name for kw in lowered_keywords) and lower_name in lowered_keywords:
            return True

    return False


def install_platform_firmware(
    api_get_json_fn,
    api_get_bytes_fn,
    platform_id: int,
    target_dirs: list,
    *,
    skip_existing: bool = True,
    extract_zip_with_paths: bool = False,
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
            _logger.debug("No target directory matched for firmware file: %s (skipped)", file_name)
            continue

        keep_archive = False
        lower_name = file_name.lower()
        if not extract_zip_with_paths and lower_name.endswith(".zip"):
            keep_archive = should_keep_zip_archive(file_name, target_dirs, applicable_dirs)

        try:
            _logger.debug("Fetching firmware file: %s", file_name)
            data = download_firmware_bytes(api_get_bytes_fn, firmware_id, file_name)
            _logger.debug("Downloaded %d bytes for: %s", len(data), file_name)
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

            if lower_name.endswith(".7z") or lower_name.endswith(".rar"):
                try:
                    from grid_launcher.library.archive_preparation import extract_archive_into_directory

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
                                    _logger.debug("Firmware file already exists, skipping: %s", member_dest)
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
                if keep_archive and lower_name.endswith(".zip"):
                    if skip_existing and dest_path.exists():
                        continue
                    try:
                        dest_path.write_bytes(data)
                        _logger.debug("Installed firmware: %s -> %s", file_name, dest_path)
                    except OSError as error:
                        warnings.append(f"Failed to write firmware {file_name} to {dest_path}: {error}")
                    continue
                try:
                    with zipfile.ZipFile(io.BytesIO(data)) as zf:
                        for member in zf.namelist():
                            if member.endswith("/") or member.startswith("__MACOSX"):
                                continue
                            if extract_zip_with_paths:
                                normalized_member = member.replace("\\", "/")
                                normalized = Path(normalized_member)
                                parts = normalized.parts
                                if not parts or any(part == ".." for part in parts) or normalized.is_absolute():
                                    continue
                                member_dest = target_dir / normalized
                                if skip_existing and member_dest.exists():
                                    _logger.debug("Firmware file already exists, skipping: %s", member_dest)
                                    continue
                                member_dest.parent.mkdir(parents=True, exist_ok=True)
                                member_dest.write_bytes(zf.read(member))
                                _logger.debug("Extracted firmware member: %s -> %s", member, member_dest)
                            else:
                                extracted_name = Path(member).name
                                if not extracted_name:
                                    continue
                                member_dest = target_dir / extracted_name
                                if skip_existing and member_dest.exists():
                                    _logger.debug("Firmware file already exists, skipping: %s", member_dest)
                                    continue
                                member_dest.write_bytes(zf.read(member))
                except (zipfile.BadZipFile, OSError, KeyError) as error:
                    warnings.append(f"Failed to extract firmware archive {file_name}: {error}")
                    _logger.debug("Zip extraction error for %s: %s", file_name, error)
                    continue
            else:
                if skip_existing and dest_path.exists():
                    continue
                try:
                    dest_path.write_bytes(data)
                    _logger.debug("Installed firmware: %s -> %s", file_name, dest_path)
                except OSError as error:
                    warnings.append(f"Failed to write firmware {file_name} to {dest_path}: {error}")
                    continue

    return warnings


def _make_sony_ssl_context() -> ssl.SSLContext:
    """Return an SSL context suitable for Sony's PS3 update servers.

    Sony's PS3 update CDN (fus01.ps3.update.playstation.net) presents a
    certificate whose hostname does not match, so hostname verification is
    disabled.  The connection is still encrypted; only the identity check is
    skipped for this well-known endpoint.
    """
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE
    return ctx


def fetch_ps3_firmware_url() -> str:
    """Return the CDN URL for the latest PS3UPDAT.PUP from Sony's manifest.

    Fetches ``PS3_FIRMWARE_MANIFEST_URL`` and parses the line that contains
    ``PS3UPDAT.PUP`` (excluding patch files) to extract the ``CDN=`` value.

    Raises ``ValueError`` if the URL cannot be found in the manifest.
    """
    ctx = _make_sony_ssl_context()
    with urllib.request.urlopen(PS3_FIRMWARE_MANIFEST_URL, context=ctx, timeout=15) as resp:
        text = resp.read().decode("utf-8", errors="replace")

    for line in text.splitlines():
        if "PS3UPDAT.PUP" not in line:
            continue
        if "PS3PATCH.PUP" in line:
            continue
        for part in line.split(";"):
            if part.startswith("CDN="):
                url = part[4:].strip()
                if url:
                    _logger.debug("Resolved PS3UPDAT.PUP URL from Sony manifest: %s", url)
                    return url

    raise ValueError("PS3UPDAT.PUP URL not found in Sony firmware manifest")


def download_ps3_firmware_direct(
    target_dirs: list,
    *,
    skip_existing: bool = True,
    progress_callback: Callable[[int, int, float], None] | None = None,
) -> list[str]:
    """Download PS3UPDAT.PUP directly from Sony's update servers.

    Resolves the current firmware URL from Sony's manifest, downloads the file,
    and writes it to each entry in *target_dirs* that accepts it (same format
    as :func:`install_platform_firmware`'s *target_dirs*).

    Returns a list of warning strings; an empty list means success.
    """
    if not target_dirs:
        return []

    file_name = "PS3UPDAT.PUP"
    applicable_dirs = resolve_firmware_targets(file_name, target_dirs)
    if not applicable_dirs:
        return []

    if skip_existing and all((d / file_name).exists() for d in applicable_dirs):
        _logger.debug("PS3UPDAT.PUP already present in all target directories, skipping download")
        return []

    try:
        firmware_url = fetch_ps3_firmware_url()
    except Exception as error:
        return [f"Failed to resolve PS3 firmware URL from Sony: {error}"]

    try:
        _logger.debug("Downloading PS3UPDAT.PUP from Sony: %s", firmware_url)
        ctx = _make_sony_ssl_context()
        with urllib.request.urlopen(firmware_url, context=ctx, timeout=300) as resp:
            content_length = resp.headers.get("Content-Length")
            total_bytes = int(content_length) if isinstance(content_length, str) and content_length.isdigit() else 0
            chunk_size = 65536
            downloaded = 0
            start_time = time.monotonic()
            chunks: list[bytes] = []
            while True:
                chunk = resp.read(chunk_size)
                if not chunk:
                    break
                chunks.append(chunk)
                downloaded += len(chunk)
                if progress_callback is not None:
                    elapsed = time.monotonic() - start_time
                    speed = downloaded / elapsed if elapsed > 0 else 0.0
                    progress_callback(downloaded, total_bytes, speed)
            data = b"".join(chunks)
        _logger.debug("Downloaded %d bytes for PS3UPDAT.PUP", len(data))
    except Exception as error:
        return [f"Failed to download PS3UPDAT.PUP from Sony: {error}"]

    warnings: list[str] = []
    for target_dir in applicable_dirs:
        dest_path = target_dir / file_name
        if skip_existing and dest_path.exists():
            _logger.debug("PS3UPDAT.PUP already exists, skipping: %s", dest_path)
            continue
        try:
            target_dir.mkdir(parents=True, exist_ok=True)
            dest_path.write_bytes(data)
            _logger.debug("Installed PS3UPDAT.PUP -> %s", dest_path)
        except OSError as error:
            warnings.append(f"Failed to write PS3UPDAT.PUP to {dest_path}: {error}")

    return warnings
