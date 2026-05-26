from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def detect_linux_emulators() -> list[dict[str, str]]:
    if sys.platform == "win32":
        return []

    detected: list[dict[str, str]] = []
    seen_slugs: set[str] = set()

    system_binaries: tuple[tuple[str, str, str], ...] = (
        ("/usr/bin/pcsx2-qt", "pcsx2", "PCSX2"),
        ("/usr/bin/dolphin-emu", "dolphin", "Dolphin"),
        ("/usr/bin/retroarch", "retroarch", "RetroArch"),
        ("/usr/bin/duckstation-qt", "duckstation", "DuckStation"),
        ("/usr/bin/ppsspp", "ppsspp", "PPSSPP"),
        ("/usr/bin/xemu", "xemu", "Xemu"),
        ("/usr/bin/rpcs3", "rpcs3", "RPCS3"),
    )

    for binary_path, slug, name in system_binaries:
        binary = Path(binary_path)
        if slug in seen_slugs:
            continue
        if not (binary.exists() and binary.is_file()):
            continue
        detected.append(
            {
                "name": name,
                "slug": slug,
                "path": binary_path,
                "args": "%rom%",
                "autodetected": "true",
            }
        )
        seen_slugs.add(slug)

    installed_ids: set[str]
    try:
        result = subprocess.run(
            ["flatpak", "list", "--app", "--columns=application"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        installed_ids = {
            line.strip()
            for line in result.stdout.splitlines()
            if isinstance(line, str) and line.strip()
        }
    except (OSError, subprocess.TimeoutExpired):
        installed_ids = set()

    flatpak_apps: tuple[tuple[str, str, str], ...] = (
        ("net.pcsx2.PCSX2", "pcsx2", "PCSX2"),
        ("org.DolphinEmu.dolphin-emu", "dolphin", "Dolphin"),
        ("org.duckstation.DuckStation", "duckstation", "DuckStation"),
        ("app.xemu.xemu", "xemu", "Xemu"),
        ("info.cemu.Cemu", "cemu", "Cemu"),
        ("org.ppsspp.PPSSPP", "ppsspp", "PPSSPP"),
        ("org.libretro.RetroArch", "retroarch", "RetroArch"),
        ("net.rpcs3.RPCS3", "rpcs3", "RPCS3"),
        ("org.azahar_emu.Azahar", "azahar", "Azahar"),
    )

    for app_id, slug, name in flatpak_apps:
        if slug in seen_slugs:
            continue
        if app_id not in installed_ids:
            continue
        detected.append(
            {
                "name": name,
                "slug": slug,
                "path": "/usr/bin/flatpak",
                "args": f"run {app_id} %rom%",
                "autodetected": "true",
            }
        )
        seen_slugs.add(slug)

    return detected