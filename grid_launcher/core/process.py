from __future__ import annotations

import os
import sys
from typing import Mapping


def clean_subprocess_env(base: Mapping[str, str] | None = None) -> dict[str, str]:
    """Return an environment suitable for spawning host-system binaries.

    PyInstaller bundles point LD_LIBRARY_PATH at the bundle's private library
    directory so the frozen app finds its own libraries. Host binaries spawned
    with that environment (7z, tar, emulators) can then resolve their C++
    runtime against the bundle's older libraries and fail to start with loader
    errors such as "version `CXXABI_1.3.15' not found". Restore the loader path
    PyInstaller saved in LD_LIBRARY_PATH_ORIG, or drop the variable entirely
    when running frozen without a saved original.
    """
    env = dict(os.environ if base is None else base)
    saved_original = env.get("LD_LIBRARY_PATH_ORIG")
    if saved_original is not None:
        env["LD_LIBRARY_PATH"] = saved_original
    elif getattr(sys, "frozen", False):
        env.pop("LD_LIBRARY_PATH", None)
    return env
