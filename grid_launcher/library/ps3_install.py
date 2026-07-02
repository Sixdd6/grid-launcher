from __future__ import annotations

import re
import shutil
from pathlib import Path
from typing import Callable


_PS3_GAME_ID_RE = re.compile(r"^[A-Z]{4}\d{5}$")
_NPWR_TROPHY_RE = re.compile(r"^NPWR\d{5}$")


# ---------------------------------------------------------------------------
# Public helpers shared with other modules
# ---------------------------------------------------------------------------

def ps3_game_id_from_text(value: str) -> str:
    if not isinstance(value, str):
        return ""
    match = re.search(r"([A-Z]{4}\d{5})", value.upper())
    if match is None:
        return ""
    return match.group(1)


def ps3_game_id_from_paths(paths: list[Path]) -> str:
    for path in paths:
        for part in path.parts:
            game_id = ps3_game_id_from_text(part)
            if game_id and not _NPWR_TROPHY_RE.match(game_id):
                return game_id
    return ""


def _has_ps3_game_content(directory: Path) -> bool:
    try:
        return (directory / "PS3_GAME").is_dir()
    except OSError:
        return False


def _is_disc_game_id_directory(directory: Path) -> bool:
    try:
        return (directory / "PS3_GAME").is_dir() and (directory / "PS3_DISC.SFB").is_file()
    except OSError:
        return False


def detected_ps3_game_id(extracted_dir: Path, installed_paths: list[Path]) -> str:
    candidate_paths = [extracted_dir, *installed_paths]
    game_id = ps3_game_id_from_paths(candidate_paths)
    if game_id:
        return game_id

    try:
        for candidate in extracted_dir.rglob("*"):
            if not candidate.is_dir():
                continue
            game_id = ps3_game_id_from_text(candidate.name)
            if game_id and _has_ps3_game_content(candidate):
                return game_id
    except OSError:
        pass
    return ""


# ---------------------------------------------------------------------------
# Content classification
# ---------------------------------------------------------------------------

def ps3_classify_extracted_contents(
    extracted_dir: Path,
) -> list[tuple[Path, str]]:
    """Scan extracted_dir and classify each top-level entry.

    Returns a list of (path, classification) tuples. Classifications:
            "disc_game_id_dir" — BLUS30336/ with PS3_GAME/ and PS3_DISC.SFB (disc-style extracted layout)
      "game_id_dir"      — BLUS30336/ with PS3_GAME/ inside
      "trophy_dir"       — NPWR##### directory
      "bare_disc_dir"    — PS3_GAME/ at top level (disc dump, no ID wrapper)
      "iso_file"         — *.iso file
      "nested_hdd0_game" — dev_hdd0/game/<ID>/ and/or dev_hdd0/home/ layout nested under extracted root
      "config_dir"       — config/ directory (RPCS3 per-game custom configs)
      "unknown"          — anything else
    """
    results: list[tuple[Path, str]] = []

    try:
        entries = sorted(extracted_dir.iterdir(), key=lambda p: (0 if p.is_dir() else 1, p.name.casefold()))
    except OSError:
        return results

    for entry in entries:
        name_upper = entry.name.upper()

        if entry.is_file():
            if entry.suffix.casefold() == ".iso":
                results.append((entry, "iso_file"))
            else:
                results.append((entry, "unknown"))
            continue

        if not entry.is_dir():
            continue

        # NPWR##### -> trophy dir
        if _NPWR_TROPHY_RE.match(name_upper):
            results.append((entry, "trophy_dir"))
            continue

        # BLUS30336/ with PS3_GAME inside -> game_id_dir
        if _PS3_GAME_ID_RE.match(name_upper) and _is_disc_game_id_directory(entry):
            results.append((entry, "disc_game_id_dir"))
            continue

        if _PS3_GAME_ID_RE.match(name_upper) and _has_ps3_game_content(entry):
            results.append((entry, "game_id_dir"))
            continue

        # PS3_GAME/ at root -> bare disc dump
        if name_upper == "PS3_GAME":
            results.append((entry, "bare_disc_dir"))
            continue

        # dev_hdd0/game/<ID>/ or dev_hdd0/home/ nested layout
        if name_upper == "DEV_HDD0":
            if (entry / "game").is_dir() or (entry / "home").is_dir():
                results.append((entry, "nested_hdd0_game"))
                continue

        # config/ at root -> RPCS3 per-game custom config directory
        if name_upper == "CONFIG":
            results.append((entry, "config_dir"))
            continue

        # BLUS30336/ without PS3_GAME (incomplete/unusual) still treated as game_id_dir
        if _PS3_GAME_ID_RE.match(name_upper):
            results.append((entry, "game_id_dir"))
            continue

        results.append((entry, "unknown"))

    return results


# ---------------------------------------------------------------------------
# Content routing
# ---------------------------------------------------------------------------

def ps3_route_extracted_contents(
    extracted_dir: Path,
    dev_hdd0_root: Path,
    iso_extract_fn: Callable[[Path, Path], Path],
    games_root: Path | None = None,
    rpcs3_data_root: Path | None = None,
) -> tuple[str, list[Path]]:
    """Route classified PS3 archive contents into the correct VFS destinations.

    Returns (game_id, installed_paths) where installed_paths are directories
    written under dev_hdd0_root. A config/ entry at the archive root is merged
    into <rpcs3_data_root>/config/ when provided, otherwise
    <dev_hdd0_root.parent>/config/.
    """
    classified = ps3_classify_extracted_contents(extracted_dir)
    installed_paths: list[Path] = []
    game_id = ""

    for item_path, classification in classified:
        if classification == "disc_game_id_dir":
            _id = item_path.name.upper()
            destination_root = games_root if isinstance(games_root, Path) else (dev_hdd0_root / "game")
            dest = destination_root / _id
            _copytree_merge(item_path, dest)
            installed_paths.append(dest)
            if not game_id:
                game_id = _id

        elif classification == "game_id_dir":
            _id = item_path.name.upper()
            dest = dev_hdd0_root / "game" / _id
            _copytree_merge(item_path, dest)
            installed_paths.append(dest)
            if not game_id:
                game_id = _id

        elif classification == "trophy_dir":
            dest = dev_hdd0_root / "home" / "00000001" / "trophy" / item_path.name
            _copytree_merge(item_path, dest)
            installed_paths.append(dest)

        elif classification == "bare_disc_dir":
            # PS3_GAME/ at root — we have no game ID; synthesize from SFO if possible,
            # fall back to extracting into a placeholder named "PS3_GAME_DISC"
            synthetic_id = _detect_game_id_from_sfo(item_path.parent) or "PS3_GAME_DISC"
            wrapper = item_path.parent / synthetic_id
            dest = dev_hdd0_root / "game" / synthetic_id
            # Create a proper game-ID wrapper dir in dest with PS3_GAME inside
            inner_dest = dest / "PS3_GAME"
            _copytree_merge(item_path, inner_dest)
            installed_paths.append(dest)
            if not game_id:
                game_id = synthetic_id

        elif classification == "iso_file":
            import tempfile
            with tempfile.TemporaryDirectory() as tmp_dir:
                iso_extracted = iso_extract_fn(item_path, Path(tmp_dir))
                iso_game_id, iso_paths = ps3_route_extracted_contents(
                    iso_extracted,
                    dev_hdd0_root,
                    iso_extract_fn,
                    games_root=games_root,
                    rpcs3_data_root=rpcs3_data_root,
                )
                installed_paths.extend(iso_paths)
                if not game_id and iso_game_id:
                    game_id = iso_game_id

        elif classification == "nested_hdd0_game":
            # dev_hdd0/game/<ID>/ — merge each game ID dir into dev_hdd0_root/game/
            game_dir = item_path / "game"
            if game_dir.is_dir():
                try:
                    for child in game_dir.iterdir():
                        if not child.is_dir():
                            continue
                        _id = child.name.upper()
                        dest = dev_hdd0_root / "game" / _id
                        _copytree_merge(child, dest)
                        installed_paths.append(dest)
                        if not game_id and _PS3_GAME_ID_RE.match(_id):
                            game_id = _id
                except OSError:
                    pass

            # dev_hdd0/home/ — covers trophy dirs and exdata (RAP decryption keys)
            home_dir = item_path / "home"
            if home_dir.is_dir():
                _copytree_merge(home_dir, dev_hdd0_root / "home")
                # Track individual NPWR trophy dirs so they can be cleaned up on uninstall
                trophy_base = home_dir / "00000001" / "trophy"
                if trophy_base.is_dir():
                    try:
                        for trophy_child in trophy_base.iterdir():
                            if trophy_child.is_dir() and _NPWR_TROPHY_RE.match(trophy_child.name.upper()):
                                dest = dev_hdd0_root / "home" / "00000001" / "trophy" / trophy_child.name
                                installed_paths.append(dest)
                    except OSError:
                        pass

        elif classification == "config_dir":
            # Merge config/ into rpcs3_data_root/config/ (the emulator config dir)
            effective_data_root = rpcs3_data_root if rpcs3_data_root is not None else dev_hdd0_root.parent
            dest = effective_data_root / "config"
            _copytree_merge(item_path, dest)
            installed_paths.append(dest)

        # "unknown" entries are silently skipped

    # If game_id still not detected, scan installed game dirs
    if not game_id:
        game_id = ps3_game_id_from_paths(installed_paths)

    return game_id, installed_paths


def _copytree_merge(src: Path, dst: Path) -> None:
    """Copy src tree into dst, merging with existing content."""
    dst.mkdir(parents=True, exist_ok=True)
    shutil.copytree(str(src), str(dst), dirs_exist_ok=True)


def _detect_game_id_from_sfo(parent_dir: Path) -> str:
    """Try to read PARAM.SFO from a disc dir to find the game ID."""
    sfo_candidates = [
        parent_dir / "PS3_GAME" / "PARAM.SFO",
        parent_dir / "PARAM.SFO",
    ]
    for sfo_path in sfo_candidates:
        if not sfo_path.exists() or not sfo_path.is_file():
            continue
        try:
            data = sfo_path.read_bytes()
            # PARAM.SFO: scan for TITLE_ID string value (ASCII, 9 chars like BLUS30336)
            for match in re.finditer(rb"[A-Z]{4}\d{5}", data):
                candidate = match.group(0).decode("ascii", errors="ignore")
                if _PS3_GAME_ID_RE.match(candidate):
                    return candidate
        except OSError:
            continue
    return ""


# ---------------------------------------------------------------------------
# ISO extraction
# ---------------------------------------------------------------------------

def extract_iso_to_ps3_layout(iso_path: Path, temp_dir: Path) -> Path:
    """Extract a PS3 ISO into temp_dir using 7-Zip and return temp_dir."""
    from grid_launcher.library.archive_preparation import _extract_7z_with_fallbacks
    _extract_7z_with_fallbacks(iso_path, temp_dir)
    return temp_dir
