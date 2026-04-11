from __future__ import annotations
from pathlib import Path


def ensure_ppsspp_settings(emulator_path_text: str) -> dict:
    path_text = emulator_path_text.strip() if isinstance(emulator_path_text, str) else ""
    if not path_text:
        return {"changed": False}

    emulator_path = Path(path_text).expanduser()
    emulator_dir = emulator_path if emulator_path.is_dir() else emulator_path.parent
    installed_txt = emulator_dir / "installed.txt"

    if installed_txt.exists():
        try:
            installed_txt.unlink()
            return {"changed": True}
        except OSError:
            return {"changed": False}

    return {"changed": False}
