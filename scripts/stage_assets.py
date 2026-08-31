#!/usr/bin/env python3
"""Stage the subset of ``assets/`` that the app actually uses.

The full ``assets/`` tree stays in the repo for development, but release builds
only need the files that source code can reach. This script derives that set
from the code itself (so it never drifts from reality), copies it into a
staging directory, and the build scripts pass that directory to PyInstaller as
``--add-data <staging>:assets`` so runtime paths are unchanged.

Usage:
    python scripts/stage_assets.py [--output build/bundle-assets] [--platform linux|windows]
"""

from __future__ import annotations

import argparse
import re
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
ASSETS_ROOT = REPO_ROOT / "assets"

# Source trees scanned for literal asset references.
GLYPH_SCAN_PATHS = (REPO_ROOT / "grid_launcher", REPO_ROOT / "grid-launcher.py")
SVG_SCAN_PATHS = (REPO_ROOT / "grid_launcher", REPO_ROOT / "grid-launcher.py", REPO_ROOT / "tests")

# Gamepad glyph stems are uppercase after "input_" (input_BTN-D, input_DPAD-U,
# input_LB, ...). Anchoring on an uppercase letter keeps ordinary identifiers
# such as "input_widget" or "input_changed" out of the match.
GLYPH_PATTERN = re.compile(r"input_[A-Z][A-Za-z0-9-]*")

# SVGs are referenced as "svg/play" or "svg/play.svg" and resolved by
# grid_launcher/ui/theme.py::_resolve_svg_asset_path.
SVG_PATTERN = re.compile(r"svg/([A-Za-z0-9_.-]+)")


def iter_source_files(scan_paths: tuple[Path, ...]) -> list[Path]:
    """Return every Python file under the given files/directories."""
    files: list[Path] = []
    for path in scan_paths:
        if path.is_dir():
            files.extend(sorted(path.rglob("*.py")))
        elif path.is_file():
            files.append(path)
    return files


def read_sources(scan_paths: tuple[Path, ...]) -> str:
    """Concatenate the text of every scanned source file."""
    return "\n".join(
        path.read_text(encoding="utf-8", errors="replace") for path in iter_source_files(scan_paths)
    )


def derive_platform_logo_files() -> list[str]:
    """Platform logo PNGs, taken from the single dict that can name them."""
    if str(REPO_ROOT) not in sys.path:
        sys.path.insert(0, str(REPO_ROOT))
    from grid_launcher.server.platform_metadata import PLATFORM_LOGO_FILES

    return sorted(set(PLATFORM_LOGO_FILES.values()))


def derive_gamepad_glyph_files(source_text: str | None = None) -> list[str]:
    """Gamepad glyph PNGs referenced by ControlHint literals and direct loads."""
    text = read_sources(GLYPH_SCAN_PATHS) if source_text is None else source_text
    return sorted({f"{stem}.png" for stem in GLYPH_PATTERN.findall(text)})


def derive_retroarch_files() -> list[str]:
    """Every ``assets/retroarch-assets`` file the app can load."""
    return sorted(set(derive_platform_logo_files()) | set(derive_gamepad_glyph_files()))


def derive_svg_files(source_text: str | None = None) -> list[str]:
    """Every ``assets/svg`` file referenced by a source literal."""
    text = read_sources(SVG_SCAN_PATHS) if source_text is None else source_text
    names = set()
    for name in SVG_PATTERN.findall(text):
        names.add(name if name.casefold().endswith(".svg") else f"{name}.svg")
    return sorted(names)


def plan_copies(platform: str) -> list[tuple[Path, Path]]:
    """Build the (source, destination-relative) list for the given platform."""
    copies: list[tuple[Path, Path]] = []
    for name in derive_retroarch_files():
        copies.append((ASSETS_ROOT / "retroarch-assets" / name, Path("retroarch-assets") / name))
    for name in derive_svg_files():
        copies.append((ASSETS_ROOT / "svg" / name, Path("svg") / name))
    if platform == "windows":
        # 7z.exe/7z.dll are only invoked on Windows (see archive_preparation.py).
        for source in sorted((ASSETS_ROOT / "tools" / "7z").glob("*")):
            if source.is_file():
                copies.append((source, Path("tools") / "7z" / source.name))
    return copies


def stage_assets(output_dir: Path, platform: str) -> list[tuple[Path, Path]]:
    """Recreate ``output_dir`` containing only the derived asset set."""
    output_dir = output_dir.resolve()
    if output_dir in (REPO_ROOT, ASSETS_ROOT) or output_dir in REPO_ROOT.parents:
        raise SystemExit(f"ERROR: refusing to stage into '{output_dir}'")

    copies = plan_copies(platform)
    missing = [str(source) for source, _ in copies if not source.is_file()]
    if missing:
        raise SystemExit(
            "ERROR: derived asset files are missing from the source tree:\n  "
            + "\n  ".join(missing)
        )

    shutil.rmtree(output_dir, ignore_errors=True)
    for source, relative in copies:
        destination = output_dir / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    return copies


def default_platform() -> str:
    return "windows" if sys.platform.startswith("win") else "linux"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        default=str(REPO_ROOT / "build" / "bundle-assets"),
        help="Staging directory to (re)create (default: build/bundle-assets)",
    )
    parser.add_argument(
        "--platform",
        choices=("linux", "windows"),
        default=default_platform(),
        help="Target platform; only Windows builds bundle assets/tools/7z",
    )
    args = parser.parse_args(argv)

    output_dir = Path(args.output)
    copies = stage_assets(output_dir, args.platform)

    groups = {"retroarch-assets": 0, "svg": 0, "tools/7z": 0}
    for _, relative in copies:
        groups["tools/7z" if relative.parts[0] == "tools" else relative.parts[0]] += 1
    summary = ", ".join(f"{count} {name}" for name, count in groups.items())
    print(f"Staged assets for {args.platform} into {output_dir}: {summary} ({len(copies)} files)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
