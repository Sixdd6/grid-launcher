#!/bin/bash
# Flatpak launch wrapper for grid-launcher.
# Installed to /app/bin/grid-launcher via the manifest's
# `install -Dm755 flatpak/grid-launcher.sh /app/bin/grid-launcher` build command,
# which also marks it executable (chmod 755) at Flatpak build time.
export GRID_LAUNCHER_SHARE_DIR=/app/share/grid-launcher
exec python3 /app/bin/grid-launcher-launcher.py "$@"
