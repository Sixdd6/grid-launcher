"""GRID Launcher package modules."""

try:
    from grid_launcher.version import __version__
except ImportError:  # version.py is generated at build time
    __version__ = "0.0.0-dev"

