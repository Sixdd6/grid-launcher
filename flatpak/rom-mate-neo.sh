#!/bin/bash
# Flatpak launch wrapper for rom-mate-neo.
# Installed to /app/bin/rom-mate-neo via the manifest's
# `install -Dm755 flatpak/rom-mate-neo.sh /app/bin/rom-mate-neo` build command,
# which also marks it executable (chmod 755) at Flatpak build time.
export ROM_MATE_SHARE_DIR=/app/share/rom-mate-neo
exec python3 /app/bin/rom-mate-neo-launcher.py "$@"
