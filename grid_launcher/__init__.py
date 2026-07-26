"""GRID Launcher package modules."""

import ssl

try:
    import certifi

    ssl._create_default_https_context = lambda: ssl.create_default_context(cafile=certifi.where())
except ImportError:
    pass

try:
    from grid_launcher.version import __version__
except ImportError:  # version.py is generated at build time
    __version__ = "0.0.0-dev"

